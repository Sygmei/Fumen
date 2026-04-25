use crate::config::AppConfig;
use crate::schemas::DrumMapEntry;
use anyhow::{Context, Result};
use bytes::Bytes;
use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use roxmltree::{Document, Node};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinSet;
use zip::ZipArchive;

pub const DEFAULT_STEM_QUALITY_PROFILE: &str = "standard";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StemQualityProfile {
    Standard,
    Small,
    VerySmall,
    Tiny,
}

impl StemQualityProfile {
    pub fn from_slug(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "small" | "compact" => Some(Self::Small),
            "very-small" | "very_small" | "verysmall" | "high" => Some(Self::VerySmall),
            "tiny" => Some(Self::Tiny),
            _ => None,
        }
    }

    pub fn from_stored_or_default(value: &str) -> Self {
        Self::from_slug(value).unwrap_or(Self::Standard)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Small => "small",
            Self::VerySmall => "very-small",
            Self::Tiny => "tiny",
        }
    }

    pub fn opus_bitrate(self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::Small => Some("24k"),
            Self::VerySmall => Some("16k"),
            Self::Tiny => Some("12k"),
        }
    }
}

fn extract_musescore_drumset_mappings(score_path: &Path) -> Result<DrumsetMappingMap> {
    if !score_path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mscz"))
    {
        return Ok(HashMap::new());
    }

    let file = File::open(score_path)
        .with_context(|| format!("opening MuseScore archive {}", score_path.display()))?;
    let mut archive =
        ZipArchive::new(file).with_context(|| format!("reading {}", score_path.display()))?;

    let mscx_name = (0..archive.len())
        .filter_map(|idx| {
            archive
                .by_index(idx)
                .ok()
                .map(|entry| entry.name().to_owned())
        })
        .find(|name| name.ends_with(".mscx") && !name.contains('/'))
        .or_else(|| {
            (0..archive.len())
                .filter_map(|idx| {
                    archive
                        .by_index(idx)
                        .ok()
                        .map(|entry| entry.name().to_owned())
                })
                .find(|name| name.ends_with(".mscx") && !name.starts_with("Excerpts/"))
        });

    let Some(mscx_name) = mscx_name else {
        return Ok(HashMap::new());
    };

    let mut entry = archive
        .by_name(&mscx_name)
        .with_context(|| format!("opening '{mscx_name}' in {}", score_path.display()))?;
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .with_context(|| format!("reading '{mscx_name}' in {}", score_path.display()))?;

    let document = Document::parse(&xml)
        .with_context(|| format!("parsing '{mscx_name}' in {}", score_path.display()))?;

    let mut mappings = HashMap::new();
    for part in document
        .descendants()
        .filter(|node| node.has_tag_name("Part"))
    {
        let Some(instrument) = part
            .children()
            .find(|child| child.is_element() && child.has_tag_name("Instrument"))
        else {
            continue;
        };

        let uses_drumset = instrument
            .children()
            .find(|child| child.is_element() && child.has_tag_name("useDrumset"))
            .and_then(|node| node.text())
            .is_some_and(|value| value.trim() == "1");
        if !uses_drumset {
            continue;
        }

        let drum_map: Vec<DrumMapEntry> = instrument
            .children()
            .filter(|child| child.is_element() && child.has_tag_name("Drum"))
            .filter_map(parse_musescore_drum_entry)
            .collect();
        if drum_map.is_empty() {
            continue;
        }

        let aliases = musescore_part_name_aliases(part, instrument);
        for alias in aliases {
            mappings.entry(alias).or_insert_with(|| drum_map.clone());
        }
    }

    Ok(mappings)
}

fn parse_musescore_drum_entry(drum: Node<'_, '_>) -> Option<DrumMapEntry> {
    let pitch = drum.attribute("pitch")?.trim().parse::<u8>().ok()?;
    let text_of = |tag_name: &str| {
        drum.children()
            .find(|child| child.is_element() && child.has_tag_name(tag_name))
            .and_then(|node| node.text())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };

    Some(DrumMapEntry {
        pitch,
        name: text_of("name")?,
        head: text_of("head"),
        line: text_of("line").and_then(|value| value.parse::<i8>().ok()),
        voice: text_of("voice").and_then(|value| value.parse::<u8>().ok()),
        stem: text_of("stem").and_then(|value| value.parse::<i8>().ok()),
        shortcut: text_of("shortcut"),
    })
}

fn musescore_part_name_aliases(part: Node<'_, '_>, instrument: Node<'_, '_>) -> HashSet<String> {
    let mut aliases = HashSet::new();

    for node in [part, instrument] {
        for tag_name in ["trackName", "longName", "shortName"] {
            if let Some(name) = node
                .children()
                .find(|child| child.is_element() && child.has_tag_name(tag_name))
                .and_then(|node| node.text())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                aliases.insert(normalize_track_lookup_key(name));
            }
        }
    }

    aliases
}

pub enum ConversionOutcome {
    Ready {
        bytes: Bytes,
        content_type: &'static str,
        extension: &'static str,
    },
    Unavailable {
        reason: String,
    },
    Failed {
        reason: String,
    },
}

pub struct StemResult {
    pub track_index: usize,
    pub track_name: String,
    pub instrument_name: String,
    pub bytes: Bytes,
    pub drum_map: Option<Vec<DrumMapEntry>>,
}

type DrumsetMappingMap = HashMap<String, Vec<DrumMapEntry>>;
pub type ProgressLogSender = UnboundedSender<String>;

struct TrackInfo {
    /// Index into the original MIDI track list / raw MTrk chunk list.
    midi_track_index: usize,
    track_name: String,
    program: u8,
    is_percussion: bool,
}

fn emit_progress(progress_log: Option<&ProgressLogSender>, message: impl Into<String>) {
    if let Some(progress_log) = progress_log {
        let _ = progress_log.send(message.into());
    }
}

// ---------------------------------------------------------------------------
// Public async entry points
// ---------------------------------------------------------------------------

#[tracing::instrument(skip(config, output_dir), fields(input_path = %input_path.display()))]
pub async fn generate_midi(
    config: &AppConfig,
    input_path: &Path,
    output_dir: &Path,
    progress_log: Option<&ProgressLogSender>,
) -> Result<ConversionOutcome> {
    convert_with_musescore(
        config,
        input_path,
        &output_dir.join("preview.mid"),
        "audio/midi",
        "mid",
        "MuseScore CLI not configured. Set MUSESCORE_BIN to enable MIDI conversion.",
        progress_log,
    )
    .await
}

#[tracing::instrument(skip(config, output_dir), fields(input_path = %input_path.display()))]
pub async fn generate_musicxml(
    config: &AppConfig,
    input_path: &Path,
    output_dir: &Path,
    progress_log: Option<&ProgressLogSender>,
) -> Result<ConversionOutcome> {
    convert_with_musescore(
        config,
        input_path,
        &output_dir.join("score.musicxml"),
        "application/xml",
        "musicxml",
        "MuseScore CLI not configured. Set MUSESCORE_BIN to enable MusicXML export.",
        progress_log,
    )
    .await
}

#[tracing::instrument(skip(config, output_dir), fields(input_path = %input_path.display()))]
pub async fn generate_audio(
    config: &AppConfig,
    input_path: &Path,
    output_dir: &Path,
    progress_log: Option<&ProgressLogSender>,
) -> Result<ConversionOutcome> {
    convert_with_musescore(
        config,
        input_path,
        &output_dir.join("preview.mp3"),
        "audio/mpeg",
        "mp3",
        "MuseScore CLI not configured. Set MUSESCORE_BIN to enable audio conversion.",
        progress_log,
    )
    .await
}

/// Render per-instrument OGG stems using MuseScore direct OGG export.
///
/// Reuses `output_dir/preview.mid` if it already exists from a prior
/// `generate_midi` call, avoiding a redundant MuseScore invocation.
///
/// Returns `(stems, status, error_message)`.
/// `status` is one of `"unavailable"`, `"ready"`, or `"failed"`.
#[tracing::instrument(
    skip(config, output_dir),
    fields(input_path = %input_path.display(), quality_profile = %quality_profile.as_str())
)]
pub async fn generate_stems(
    config: &AppConfig,
    input_path: &Path,
    output_dir: &Path,
    quality_profile: StemQualityProfile,
    progress_log: Option<&ProgressLogSender>,
) -> Result<(Vec<StemResult>, String, Option<String>)> {
    let profile_bitrate = quality_profile.opus_bitrate().unwrap_or("source");
    tracing::info!(
        "stems: starting pipeline for '{}' with {} profile ({})",
        input_path.file_name().unwrap_or_default().to_string_lossy(),
        quality_profile.as_str(),
        profile_bitrate,
    );
    emit_progress(
        progress_log,
        format!(
            "stems: starting pipeline for '{}' with {} profile ({})",
            input_path.file_name().unwrap_or_default().to_string_lossy(),
            quality_profile.as_str(),
            profile_bitrate,
        ),
    );

    tracing::info!("stems: renderer = musescore");
    emit_progress(progress_log, "stems: renderer = musescore");
    generate_stems_with_musescore(
        config,
        input_path,
        output_dir,
        quality_profile,
        progress_log,
    )
    .await
}

#[tracing::instrument(
    skip(config, output_dir),
    fields(input_path = %input_path.display(), quality_profile = %quality_profile.as_str())
)]
async fn generate_stems_with_musescore(
    config: &AppConfig,
    input_path: &Path,
    output_dir: &Path,
    quality_profile: StemQualityProfile,
    progress_log: Option<&ProgressLogSender>,
) -> Result<(Vec<StemResult>, String, Option<String>)> {
    if find_musescore_command(config).await.is_none() {
        return Ok((
            Vec::new(),
            "unavailable".to_owned(),
            Some(
                "MuseScore CLI not configured. Set MUSESCORE_BIN to enable direct OGG stem rendering."
                    .to_owned(),
            ),
        ));
    }

    let ffmpeg = if quality_profile == StemQualityProfile::Standard {
        None
    } else {
        match find_ffmpeg_binary().await {
            Some(binary) => Some(binary),
            None => {
                return Ok((
                    Vec::new(),
                    "unavailable".to_owned(),
                    Some(
                        "FFmpeg is required for compressed stem profiles. Install ffmpeg or use the Standard profile."
                            .to_owned(),
                    ),
                ));
            }
        }
    };

    let midi_bytes: Bytes =
        match load_or_generate_preview_midi(config, input_path, output_dir, progress_log).await? {
            ConversionOutcome::Ready { bytes, .. } => bytes,
            ConversionOutcome::Unavailable { reason } => {
                return Ok((Vec::new(), "unavailable".to_owned(), Some(reason)));
            }
            ConversionOutcome::Failed { reason } => {
                return Ok((Vec::new(), "failed".to_owned(), Some(reason)));
            }
        };

    let drumset_mappings = match extract_musescore_drumset_mappings(input_path) {
        Ok(mappings) => mappings,
        Err(error) => {
            tracing::warn!(
                "stems: failed to extract MuseScore drumset mappings from '{}': {error}",
                input_path.display()
            );
            emit_progress(
                progress_log,
                format!(
                    "stems: failed to extract MuseScore drumset mappings from '{}': {error}",
                    input_path.display()
                ),
            );
            HashMap::new()
        }
    };

    let track_infos = parse_midi_tracks(&midi_bytes);
    if track_infos.is_empty() {
        return Ok((
            Vec::new(),
            "unavailable".to_owned(),
            Some("No instrument tracks found in MIDI export".to_owned()),
        ));
    }

    let chunks = extract_raw_midi_chunks(&midi_bytes);
    if chunks.len() <= 1 {
        return Ok((
            Vec::new(),
            "unavailable".to_owned(),
            Some(
                "MIDI export is single-track (Format 0); per-instrument stems require a multi-track Format 1 export"
                    .to_owned(),
            ),
        ));
    }

    let clean_tempo_chunk = build_global_tempo_chunk(&midi_bytes);
    let total = track_infos.len();
    let max_parallel_stem_renders = config.processor_max_parallel_stem_renders.max(1);
    tracing::info!(
        "stems: found {total} instrument tracks, starting direct MuseScore render with max {max_parallel_stem_renders} concurrent render(s)"
    );
    emit_progress(
        progress_log,
        format!(
            "stems: found {total} instrument tracks, starting direct MuseScore render with max {max_parallel_stem_renders} concurrent render(s)"
        ),
    );

    let render_dir = output_dir.join("musescore-direct-stems");
    tokio::fs::create_dir_all(&render_dir)
        .await
        .with_context(|| format!("creating {}", render_dir.display()))?;

    let config = config.clone();
    let render_semaphore = Arc::new(Semaphore::new(max_parallel_stem_renders));
    enum StemJobOutcome {
        Ready(StemResult),
        Unavailable(String),
        Failed(String),
    }

    let mut jobs = JoinSet::new();
    for (stem_idx, track_info) in track_infos.iter().enumerate() {
        let chunk_idx = track_info.midi_track_index;
        let Some(chunk) = chunks.get(chunk_idx) else {
            continue;
        };

        let stem_mid_path = render_dir.join(format!("stem_{stem_idx}.mid"));
        let stem_ogg_path = render_dir.join(format!("stem_{stem_idx}.ogg"));
        let stem_compressed_path = render_dir.join(format!("stem_{stem_idx}.compressed.ogg"));
        let stem_midi = build_stem_midi(&midi_bytes, &clean_tempo_chunk, chunk);
        let track_name = track_info.track_name.clone();
        let track_label = track_name.clone();
        let program = track_info.program;
        let instrument_name =
            gm_instrument_name(track_info.program, track_info.is_percussion).to_owned();
        let drum_map = track_info.is_percussion.then(|| {
            drumset_mappings
                .get(&normalize_track_lookup_key(&track_info.track_name))
                .cloned()
                .unwrap_or_default()
        });
        let ffmpeg = ffmpeg.clone();
        let quality_profile = quality_profile;
        let config = config.clone();
        let progress_log = progress_log.cloned();
        let render_semaphore = render_semaphore.clone();

        jobs.spawn(async move {
            let _permit = render_semaphore
                .acquire_owned()
                .await
                .context("stem render semaphore closed")?;

            tokio::fs::write(&stem_mid_path, &stem_midi)
                .await
                .with_context(|| format!("writing {}", stem_mid_path.display()))?;

            tracing::info!(
                "stems: [{}/{}] '{}' ({}, GM prog {}) - rendering via MuseScore",
                stem_idx + 1,
                total,
                track_label,
                instrument_name,
                program,
            );
            emit_progress(
                progress_log.as_ref(),
                format!(
                    "stems: [{}/{}] '{}' ({}, GM prog {}) - rendering via MuseScore",
                    stem_idx + 1,
                    total,
                    track_label,
                    instrument_name,
                    program,
                ),
            );

            match convert_with_musescore(
                &config,
                &stem_mid_path,
                &stem_ogg_path,
                "audio/ogg",
                "ogg",
                "MuseScore CLI not configured. Set MUSESCORE_BIN to enable direct OGG stem rendering.",
                progress_log.as_ref(),
            )
            .await?
            {
                ConversionOutcome::Ready { bytes, .. } => {
                    let bytes = if let Some(ffmpeg_binary) = ffmpeg.as_deref() {
                        recompress_ogg_stem(
                            ffmpeg_binary,
                            &stem_ogg_path,
                            &stem_compressed_path,
                            quality_profile,
                        )
                        .await?
                    } else {
                        bytes
                    };

                    Ok(StemJobOutcome::Ready(StemResult {
                        track_index: stem_idx,
                        track_name,
                        instrument_name,
                        bytes,
                        drum_map,
                    }))
                }
                ConversionOutcome::Unavailable { reason } => {
                    Ok(StemJobOutcome::Unavailable(reason))
                }
                ConversionOutcome::Failed { reason } => {
                    tracing::warn!(
                        "stems: [{}/{}] '{}' - MuseScore direct render failed: {}",
                        stem_idx + 1,
                        total,
                        track_label,
                        reason
                    );
                    emit_progress(
                        progress_log.as_ref(),
                        format!(
                            "stems: [{}/{}] '{}' - MuseScore direct render failed: {}",
                            stem_idx + 1,
                            total,
                            track_label,
                            reason
                        ),
                    );
                    Ok(StemJobOutcome::Failed(reason))
                }
            }
        });
    }

    let mut stems = Vec::new();
    let mut first_failure = None;
    let mut unavailable_reason = None;

    while let Some(job) = jobs.join_next().await {
        match job {
            Ok(Ok(StemJobOutcome::Ready(stem))) => stems.push(stem),
            Ok(Ok(StemJobOutcome::Unavailable(reason))) => {
                if unavailable_reason.is_none() {
                    unavailable_reason = Some(reason);
                }
            }
            Ok(Ok(StemJobOutcome::Failed(reason))) => {
                if first_failure.is_none() {
                    first_failure = Some(reason);
                }
            }
            Ok(Err(error)) => return Err(error),
            Err(error) => return Err(error.into()),
        }
    }

    if let Some(reason) = unavailable_reason {
        return Ok((Vec::new(), "unavailable".to_owned(), Some(reason)));
    }

    stems.sort_by_key(|stem| stem.track_index);

    if stems.is_empty() {
        Ok((
            Vec::new(),
            "failed".to_owned(),
            Some(first_failure.unwrap_or_else(|| {
                "No stems could be rendered with MuseScore direct OGG rendering.".to_owned()
            })),
        ))
    } else {
        Ok((stems, "ready".to_owned(), None))
    }
}

// ---------------------------------------------------------------------------
// MIDI helpers
// ---------------------------------------------------------------------------

/// Parse per-track metadata (name, GM program, percussion flag) from MIDI bytes.
/// Includes ALL tracks — MuseScore sometimes puts the first instrument's notes
/// in the conductor track (track 0) instead of a clean tempo-only track.
fn parse_midi_tracks(midi_bytes: &[u8]) -> Vec<TrackInfo> {
    let smf = match Smf::parse(midi_bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("MIDI parse error: {e}");
            return Vec::new();
        }
    };

    let mut result = Vec::new();

    for (i, track) in smf.tracks.iter().enumerate() {
        let mut track_name = format!("Track {i}");
        let mut program: u8 = 0;
        let mut program_set = false;
        let mut is_percussion = false;
        let mut has_notes = false;

        for event in track {
            match &event.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
                    let name = String::from_utf8_lossy(bytes).trim().to_owned();
                    if !name.is_empty() {
                        track_name = name;
                    }
                }
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::ProgramChange { program: prog },
                } => {
                    let ch = u8::from(*channel);
                    if ch == 9 {
                        is_percussion = true;
                    } else if !program_set {
                        // Use the FIRST program change as the canonical instrument
                        program = u8::from(*prog);
                        program_set = true;
                    }
                }
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { vel, .. },
                } => {
                    if u8::from(*vel) > 0 {
                        has_notes = true;
                    }
                    if u8::from(*channel) == 9 {
                        is_percussion = true;
                    }
                }
                TrackEventKind::Midi { channel, .. } => {
                    if u8::from(*channel) == 9 {
                        is_percussion = true;
                    }
                }
                _ => {}
            }
        }

        // Only include tracks that actually have notes to render
        if !has_notes {
            continue;
        }

        result.push(TrackInfo {
            midi_track_index: i,
            track_name,
            program,
            is_percussion,
        });
    }

    result
}

/// Return a slice into `midi_bytes` for each MTrk chunk (including its 8-byte header).
fn extract_raw_midi_chunks(midi_bytes: &[u8]) -> Vec<&[u8]> {
    let mut chunks = Vec::new();
    let mut pos = 0;
    while pos + 8 <= midi_bytes.len() {
        let tag = &midi_bytes[pos..pos + 4];
        let length = u32::from_be_bytes([
            midi_bytes[pos + 4],
            midi_bytes[pos + 5],
            midi_bytes[pos + 6],
            midi_bytes[pos + 7],
        ]) as usize;
        let end = pos + 8 + length;
        if end > midi_bytes.len() {
            break;
        }
        if tag == b"MTrk" {
            chunks.push(&midi_bytes[pos..end]);
        }
        pos = end;
    }
    chunks
}

/// Return a new MTrk chunk that contains only meta events from `chunk`, stripping
/// all MIDI channel messages (Note On/Off, Program Change, Control Change, etc.).
/// This prevents MuseScore's first-instrument notes from bleeding into every stem
/// when the conductor/tempo track also carries instrument data.
fn strip_channel_events(chunk: &[u8]) -> Vec<u8> {
    if chunk.len() < 8 || &chunk[0..4] != b"MTrk" {
        return chunk.to_vec();
    }

    let mut out_events: Vec<u8> = Vec::new();
    let mut p = 8usize; // skip 8-byte MTrk header
    let end = chunk.len();
    let mut running_status: u8 = 0;

    while p < end {
        // Read VLQ delta time
        let delta_start = p;
        loop {
            if p >= end {
                break;
            }
            let b = chunk[p];
            p += 1;
            if b & 0x80 == 0 {
                break;
            }
        }
        let delta_bytes = &chunk[delta_start..p];

        if p >= end {
            break;
        }

        let b0 = chunk[p];

        if b0 == 0xFF {
            // Meta event — KEEP
            let ev_start = p;
            p += 2; // 0xFF + type byte
            // VLQ length
            let mut meta_len: usize = 0;
            loop {
                if p >= end {
                    break;
                }
                let b = chunk[p];
                p += 1;
                meta_len = (meta_len << 7) | ((b & 0x7F) as usize);
                if b & 0x80 == 0 {
                    break;
                }
            }
            p += meta_len;
            out_events.extend_from_slice(delta_bytes);
            out_events.extend_from_slice(&chunk[ev_start..p]);
            running_status = 0;
        } else if b0 == 0xF0 || b0 == 0xF7 {
            // SysEx — SKIP
            p += 1;
            let mut slen: usize = 0;
            loop {
                if p >= end {
                    break;
                }
                let b = chunk[p];
                p += 1;
                slen = (slen << 7) | ((b & 0x7F) as usize);
                if b & 0x80 == 0 {
                    break;
                }
            }
            p += slen;
            running_status = 0;
        } else {
            // MIDI channel event — SKIP (do not emit)
            if b0 & 0x80 != 0 {
                running_status = b0;
                p += 1;
            }
            let cmd = running_status & 0xF0;
            let data_bytes: usize = match cmd {
                0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => 2,
                0xC0 | 0xD0 => 1,
                _ => 0,
            };
            p += data_bytes;
        }
    }

    // Wrap filtered events back into a valid MTrk chunk
    let mut result = Vec::with_capacity(8 + out_events.len());
    result.extend_from_slice(b"MTrk");
    let len = out_events.len() as u32;
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(&out_events);
    result
}

/// Assemble a 2-track Format-1 MIDI from a tempo chunk and one instrument chunk.
fn build_stem_midi(original: &[u8], tempo_chunk: &[u8], instrument_chunk: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(14 + tempo_chunk.len() + instrument_chunk.len());
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&[0, 0, 0, 6]); // header length = 6
    out.extend_from_slice(&[0, 1]); // format = 1 (multi-track)
    out.extend_from_slice(&[0, 2]); // 2 tracks
    // Timing division from original MThd bytes 12-13
    if original.len() >= 14 {
        out.extend_from_slice(&original[12..14]);
    } else {
        out.extend_from_slice(&[0x01, 0xE0]); // 480 ticks per quarter note
    }
    out.extend_from_slice(tempo_chunk);
    out.extend_from_slice(instrument_chunk);
    out
}

#[tracing::instrument(skip(config, output_dir), fields(input_path = %input_path.display()))]
async fn load_or_generate_preview_midi(
    config: &AppConfig,
    input_path: &Path,
    output_dir: &Path,
    progress_log: Option<&ProgressLogSender>,
) -> Result<ConversionOutcome> {
    let midi_path = output_dir.join("preview.mid");
    tracing::info!("stems: exporting MIDI from score");
    emit_progress(progress_log, "stems: exporting MIDI from score");

    if midi_path.exists() {
        tracing::info!("stems: reusing existing MIDI file");
        emit_progress(progress_log, "stems: reusing existing MIDI file");
        let bytes = tokio::fs::read(&midi_path)
            .await
            .context("reading existing MIDI file")?;
        return Ok(ConversionOutcome::Ready {
            bytes: bytes.into(),
            content_type: "audio/midi",
            extension: "mid",
        });
    }

    let outcome = generate_midi(config, input_path, output_dir, progress_log).await?;
    if matches!(outcome, ConversionOutcome::Ready { .. }) {
        tracing::info!("stems: MIDI export complete");
        emit_progress(progress_log, "stems: MIDI export complete");
    }
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// Tool / path discovery
// ---------------------------------------------------------------------------

async fn find_musescore_binary(config: &AppConfig) -> Option<String> {
    if let Some(path) = &config.musescore_bin {
        return Some(path.clone());
    }

    let candidates = [
        "MuseScoreStudio.exe",
        "MuseScore4.exe",
        "MuseScore3.exe",
        "musescore",
        "mscore",
    ];

    for candidate in candidates {
        if probe_musescore_binary(candidate).await {
            return Some(candidate.to_owned());
        }
    }

    for candidate in platform_musescore_binary_candidates() {
        if probe_musescore_binary(&candidate).await {
            return Some(candidate);
        }
    }

    None
}

#[derive(Clone, Debug)]
enum MuseScoreCommand {
    Native { binary: String },
    Docker { image: String },
}

async fn probe_musescore_binary(binary: &str) -> bool {
    Command::new(binary)
        .arg("--long-version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

async fn find_ffmpeg_binary() -> Option<String> {
    let candidates = ["ffmpeg", "ffmpeg.exe"];

    for candidate in candidates {
        let output = Command::new(candidate).arg("-version").output().await;
        if output
            .as_ref()
            .is_ok_and(|command_output| command_output.status.success())
        {
            return Some(candidate.to_owned());
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn platform_musescore_binary_candidates() -> Vec<String> {
    vec![
        r"C:\Program Files\MuseScore Studio 4\bin\MuseScoreStudio.exe".to_owned(),
        r"C:\Program Files\MuseScore 4\bin\MuseScore4.exe".to_owned(),
        r"C:\Program Files\MuseScore 3\bin\MuseScore3.exe".to_owned(),
    ]
}

#[cfg(target_os = "macos")]
fn platform_musescore_binary_candidates() -> Vec<String> {
    vec![
        "/Applications/MuseScore Studio 4.app/Contents/MacOS/mscore".to_owned(),
        "/Applications/MuseScore 4.app/Contents/MacOS/mscore".to_owned(),
        "/Applications/MuseScore 3.app/Contents/MacOS/mscore".to_owned(),
    ]
}

#[cfg(target_os = "linux")]
fn platform_musescore_binary_candidates() -> Vec<String> {
    vec![
        "/usr/local/bin/musescore4".to_owned(),
        "/usr/local/bin/musescore".to_owned(),
        "/usr/bin/musescore4".to_owned(),
        "/usr/bin/musescore".to_owned(),
        "/usr/bin/mscore".to_owned(),
        "/opt/musescore4/AppRun".to_owned(),
    ]
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_musescore_binary_candidates() -> Vec<String> {
    Vec::new()
}

async fn find_musescore_command(config: &AppConfig) -> Option<MuseScoreCommand> {
    if let Some(image) = &config.musescore_docker_image {
        return Some(MuseScoreCommand::Docker {
            image: image.clone(),
        });
    }

    find_musescore_binary(config)
        .await
        .map(|binary| MuseScoreCommand::Native { binary })
}

fn native_musescore_command(binary: &str, config: &AppConfig, xdg_runtime_dir: &Path) -> Command {
    let mut command = Command::new(binary);

    #[cfg(target_os = "linux")]
    {
        command
            .env("LANG", "C.UTF-8")
            .env("LC_ALL", "C.UTF-8")
            .env("XDG_RUNTIME_DIR", xdg_runtime_dir);

        let qt_qpa_platform = config
            .musescore_qt_platform
            .clone()
            .unwrap_or_else(|| "offscreen".to_owned());
        command.env("QT_QPA_PLATFORM", qt_qpa_platform);
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = xdg_runtime_dir;
        if let Some(qt_qpa_platform) = &config.musescore_qt_platform {
            command.env("QT_QPA_PLATFORM", qt_qpa_platform);
        } else {
            command.env_remove("QT_QPA_PLATFORM");
        }
    }

    command
}

fn docker_musescore_command(
    config: &AppConfig,
    image: &str,
    input_path: &Path,
    output_path: &Path,
) -> Result<Command> {
    let input_dir = input_path
        .parent()
        .context("MuseScore input path has no parent directory")?
        .canonicalize()
        .with_context(|| format!("failed to resolve input directory {}", input_path.display()))?;
    let output_dir = output_path
        .parent()
        .context("MuseScore output path has no parent directory")?
        .canonicalize()
        .with_context(|| {
            format!(
                "failed to resolve output directory {}",
                output_path.display()
            )
        })?;

    let input_file_name = file_name(input_path)?;
    let output_file_name = file_name(output_path)?;

    let mut command = Command::new(&config.docker_bin);
    command
        .arg("run")
        .arg("--rm")
        .arg("--mount")
        .arg(bind_mount_arg(&input_dir, "/work/input", true))
        .arg("--mount")
        .arg(bind_mount_arg(&output_dir, "/work/output", false))
        .arg(image)
        .arg("-o")
        .arg(format!("/work/output/{output_file_name}"))
        .arg(format!("/work/input/{input_file_name}"));
    Ok(command)
}

fn bind_mount_arg(source: &Path, target: &str, readonly: bool) -> String {
    let mut arg = format!("type=bind,source={},target={target}", source.display());
    if readonly {
        arg.push_str(",readonly");
    }
    arg
}

fn file_name(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(OsStr::to_str)
        .with_context(|| format!("path '{}' has no valid UTF-8 file name", path.display()))
}

// ---------------------------------------------------------------------------
// MuseScore conversion helper
// ---------------------------------------------------------------------------

#[tracing::instrument(
    skip(config),
    fields(
        input_path = %input_path.display(),
        output_path = %output_path.display(),
        format = extension
    )
)]
async fn convert_with_musescore(
    config: &AppConfig,
    input_path: &Path,
    output_path: &Path,
    content_type: &'static str,
    extension: &'static str,
    unavailable_reason: &str,
    progress_log: Option<&ProgressLogSender>,
) -> Result<ConversionOutcome> {
    let Some(command_kind) = find_musescore_command(config).await else {
        return Ok(ConversionOutcome::Unavailable {
            reason: unavailable_reason.to_owned(),
        });
    };

    tracing::info!(
        "musescore: converting '{}' → {}",
        input_path.file_name().unwrap_or_default().to_string_lossy(),
        output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
    );
    emit_progress(
        progress_log,
        format!(
            "convert_with_musescore{{content_type=\"{}\" extension=\"{}\" input_path={} output_path={} format=\"{}\"}}: musescore: converting '{}' -> {}",
            content_type,
            extension,
            input_path.display(),
            output_path.display(),
            extension,
            input_path.file_name().unwrap_or_default().to_string_lossy(),
            output_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
        ),
    );

    let xdg_runtime_dir =
        tempfile::tempdir().context("failed to create MuseScore runtime directory")?;

    let (runner_label, mut command) = match &command_kind {
        MuseScoreCommand::Native { binary } => {
            let mut command = native_musescore_command(binary, config, xdg_runtime_dir.path());
            command.arg("-o").arg(output_path).arg(input_path);
            (binary.as_str().to_owned(), command)
        }
        MuseScoreCommand::Docker { image } => (
            format!("{} run {}", config.docker_bin, image),
            docker_musescore_command(config, image, input_path, output_path)?,
        ),
    };

    let command_output = command
        .output()
        .await
        .with_context(|| format!("failed to start MuseScore converter '{runner_label}'"))?;

    if !command_output.status.success() {
        let stderr =
            sanitize_musescore_output(String::from_utf8_lossy(&command_output.stderr).as_ref());
        let stdout =
            sanitize_musescore_output(String::from_utf8_lossy(&command_output.stdout).as_ref());
        let status = command_output
            .status
            .code()
            .map(|code| format!("exit code {code}"))
            .unwrap_or_else(|| "terminated by signal".to_owned());
        let detail = match (stdout.is_empty(), stderr.is_empty()) {
            (false, false) => format!("stdout:\n{stdout}\nstderr:\n{stderr}"),
            (false, true) => stdout,
            (true, false) => stderr,
            (true, true) => String::new(),
        };

        return Ok(ConversionOutcome::Failed {
            reason: if detail.is_empty() {
                format!("MuseScore converter '{runner_label}' failed with {status}.")
            } else {
                format!("MuseScore converter '{runner_label}' failed with {status}.\n{detail}")
            },
        });
    }

    let bytes = tokio::fs::read(output_path)
        .await
        .with_context(|| format!("failed to read generated file at {}", output_path.display()))?;

    tracing::info!("musescore: done ({} KB)", bytes.len() / 1024,);
    emit_progress(
        progress_log,
        format!(
            "convert_with_musescore{{content_type=\"{}\" extension=\"{}\" input_path={} output_path={} format=\"{}\"}}: musescore: done ({} KB)",
            content_type,
            extension,
            input_path.display(),
            output_path.display(),
            extension,
            bytes.len() / 1024,
        ),
    );

    Ok(ConversionOutcome::Ready {
        bytes: Bytes::from(bytes),
        content_type,
        extension,
    })
}

fn sanitize_musescore_output(output: &str) -> String {
    output
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && trimmed != "/lib/x86_64-linux-gnu/libOpenGL.so.0"
                && trimmed != "/lib/x86_64-linux-gnu/libjack.so.0"
                && trimmed != "/lib/x86_64-linux-gnu/libnss3.so"
                && trimmed
                    != "findlib: libpipewire-0.3.so.0: cannot open shared object file: No such file or directory"
                && trimmed != "/opt/musescore4/AppRun: Using fallback for library 'libpipewire-0.3.so.0'"
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tracing::instrument(
    fields(
        ffmpeg_binary = ffmpeg_binary,
        input_path = %input_path.display(),
        output_path = %output_path.display(),
        quality_profile = %quality_profile.as_str()
    )
)]
async fn recompress_ogg_stem(
    ffmpeg_binary: &str,
    input_path: &Path,
    output_path: &Path,
    quality_profile: StemQualityProfile,
) -> Result<Bytes> {
    let Some(bitrate) = quality_profile.opus_bitrate() else {
        return tokio::fs::read(input_path)
            .await
            .map(Bytes::from)
            .with_context(|| format!("failed to read {}", input_path.display()));
    };

    let command_output = Command::new(ffmpeg_binary)
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-vn")
        .arg("-c:a")
        .arg("libopus")
        .arg("-b:a")
        .arg(bitrate)
        .arg("-vbr")
        .arg("on")
        .arg("-compression_level")
        .arg("10")
        .arg(output_path)
        .output()
        .await
        .with_context(|| format!("failed to start ffmpeg recompression via '{ffmpeg_binary}'"))?;

    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr)
            .trim()
            .to_owned();
        let stdout = String::from_utf8_lossy(&command_output.stdout)
            .trim()
            .to_owned();
        let detail = match (stdout.is_empty(), stderr.is_empty()) {
            (false, false) => format!("stdout:\n{stdout}\nstderr:\n{stderr}"),
            (false, true) => stdout,
            (true, false) => stderr,
            (true, true) => String::new(),
        };
        anyhow::bail!(
            "FFmpeg failed to compress stem at {} for profile '{}'.{}{}",
            input_path.display(),
            quality_profile.as_str(),
            if detail.is_empty() { "" } else { "\n" },
            detail
        );
    }

    tokio::fs::read(output_path)
        .await
        .map(Bytes::from)
        .with_context(|| format!("failed to read {}", output_path.display()))
}

fn gm_instrument_name(program: u8, is_percussion: bool) -> &'static str {
    if is_percussion {
        return "Percussion";
    }
    match program {
        0 => "Acoustic Piano",
        1 => "Bright Piano",
        2 => "Electric Grand Piano",
        3 => "Honky-tonk Piano",
        4 => "Electric Piano 1",
        5 => "Electric Piano 2",
        6 => "Harpsichord",
        7 => "Clavinet",
        9 => "Glockenspiel",
        10 => "Music Box",
        11 => "Vibraphone",
        12 => "Marimba",
        13 => "Xylophone",
        14 => "Tubular Bells",
        16 => "Drawbar Organ",
        17 => "Percussive Organ",
        18 => "Rock Organ",
        19 => "Church Organ",
        20 => "Reed Organ",
        40 => "Violin",
        41 => "Viola",
        42 => "Cello",
        43 => "Contrabass",
        44 => "Tremolo Strings",
        45 => "Pizzicato Strings",
        46 => "Orchestral Harp",
        47 => "Timpani",
        48 => "String Ensemble 1",
        49 => "String Ensemble 2",
        50 => "Synth Strings 1",
        51 => "Synth Strings 2",
        52 => "Choir Aahs",
        53 => "Voice Oohs",
        56 => "Trumpet",
        57 => "Trombone",
        58 => "Tuba",
        59 => "Muted Trumpet",
        60 => "French Horn",
        61 => "Brass Section",
        62 => "Synth Brass 1",
        63 => "Synth Brass 2",
        64 => "Soprano Sax",
        65 => "Alto Sax",
        66 => "Tenor Sax",
        67 => "Baritone Sax",
        68 => "Oboe",
        69 => "English Horn",
        70 => "Bassoon",
        71 => "Clarinet",
        72 => "Piccolo",
        73 => "Flute",
        74 => "Recorder",
        75 => "Pan Flute",
        76 => "Blown Bottle",
        77 => "Shakuhachi",
        78 => "Whistle",
        79 => "Ocarina",
        _ => "Instrument",
    }
}

fn build_global_tempo_map(smf: &Smf<'_>) -> Vec<(u32, u32)> {
    let mut map: Vec<(u32, u32)> = vec![(0, 500_000)];
    let mut tempo_events: Vec<(u32, u32)> = Vec::new();

    for track in &smf.tracks {
        let mut abs = 0u32;
        for ev in track {
            abs = abs.saturating_add(u32::from(ev.delta));
            if let TrackEventKind::Meta(MetaMessage::Tempo(t)) = &ev.kind {
                tempo_events.push((abs, u32::from(*t)));
            }
        }
    }

    tempo_events.sort_by_key(|(tick, _)| *tick);
    for (tick, us) in tempo_events {
        if let Some(last) = map.last_mut() {
            if last.0 == tick {
                last.1 = us;
                continue;
            }
            if last.1 == us {
                continue;
            }
        }
        map.push((tick, us));
    }

    map
}

/// Build a clean MTrk chunk that carries the global tempo map for the score.
/// Tempo events are collected from all tracks because some MuseScore exports do
/// not store them in raw track 0.
fn build_global_tempo_chunk(midi_bytes: &[u8]) -> Vec<u8> {
    let smf = match Smf::parse(midi_bytes) {
        Ok(smf) => smf,
        Err(error) => {
            tracing::warn!(
                "stems: could not parse MIDI tempo map, falling back to track 0 meta events: {error}"
            );
            return extract_raw_midi_chunks(midi_bytes)
                .first()
                .map(|chunk| strip_channel_events(chunk))
                .unwrap_or_else(|| build_mtrk(Vec::new()));
        }
    };

    let events = build_global_tempo_map(&smf)
        .into_iter()
        .filter_map(|(tick, us_per_qn)| {
            if tick == 0 && us_per_qn == 500_000 {
                return None;
            }

            let bytes = us_per_qn.to_be_bytes();
            Some((tick, vec![0xFF, 0x51, 0x03, bytes[1], bytes[2], bytes[3]]))
        })
        .collect();

    build_mtrk(events)
}

/// Pack a sorted list of `(abs_tick, event_bytes)` pairs into a valid MTrk
/// chunk.  Delta times are re-computed from the absolute-tick values.
/// An EndOfTrack meta event is appended automatically.
fn build_mtrk(events: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    let mut prev = 0u32;
    for (tick, ev) in &events {
        vlq_write(&mut body, tick.saturating_sub(prev));
        body.extend_from_slice(ev);
        prev = *tick;
    }
    vlq_write(&mut body, 0);
    body.extend_from_slice(&[0xFF, 0x2F, 0x00]); // EndOfTrack
    let mut chunk = Vec::with_capacity(8 + body.len());
    chunk.extend_from_slice(b"MTrk");
    chunk.extend_from_slice(&(body.len() as u32).to_be_bytes());
    chunk.extend_from_slice(&body);
    chunk
}

/// MIDI variable-length quantity (VLQ) encoder.
fn vlq_write(buf: &mut Vec<u8>, v: u32) {
    if v < 0x80 {
        buf.push(v as u8);
        return;
    }
    let mut b = [0u8; 4];
    let mut n = 0usize;
    let mut r = v;
    while r > 0 {
        b[n] = (r & 0x7F) as u8;
        n += 1;
        r >>= 7;
    }
    for i in (0..n).rev() {
        buf.push(if i > 0 { b[i] | 0x80 } else { b[i] });
    }
}

fn normalize_track_lookup_key(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '♭' => normalized.push('b'),
            '♯' => normalized.push_str("sharp"),
            '#' => normalized.push_str("sharp"),
            _ if ch.is_alphanumeric() => normalized.extend(ch.to_lowercase()),
            _ => {}
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
    }
    #[test]
    fn extracts_drumset_mappings_from_musescore_archive() {
        let mappings = extract_musescore_drumset_mappings(&fixture_path(
            "data/storage/scores/068b3354-2c69-4691-8359-7bfb90c026f5/Chrono_Trigger_-_Main_Theme.mscz",
        ))
        .expect("fixture mscz should be readable");

        let drumset = mappings
            .get(&normalize_track_lookup_key("Set de batterie"))
            .expect("drumset mapping should exist");

        assert!(
            drumset
                .iter()
                .any(|entry| entry.pitch == 38 && entry.name == "Snare")
        );
        assert!(
            drumset
                .iter()
                .any(|entry| entry.pitch == 49 && entry.name == "Crash Cymbal")
        );
    }

    #[test]
    fn normalize_track_lookup_key_handles_accidentals_and_spacing() {
        assert_eq!(
            normalize_track_lookup_key("Clarinette en Si♭ 1"),
            normalize_track_lookup_key("Clarinette en Sib 1")
        );
        assert_eq!(
            normalize_track_lookup_key("Trompette en Si♭"),
            normalize_track_lookup_key("Trompette en Sib")
        );
    }
}
