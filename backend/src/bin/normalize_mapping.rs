//! normalize_mapping — measure the peak level of every SFZ instrument at MIDI
//! velocity 80 (mezzo-forte) and write per-instrument gain corrections back
//! into `mapping.json` under a `"gains"` key.
//!
//! Usage (run from the workspace root or the backend directory):
//!
//!   cargo run --bin normalize_mapping [path/to/mapping.json] [output/samples/dir]
//!
//! If no mapping path is given it tries `../soundfonts/mapping.json`.
//! If an output directory is given, raw WAVs are copied there and a second
//! set of gain-corrected WAVs is written alongside them for listening comparison.
//!
//! The test note is chosen from the middle of each instrument's own SFZ key
//! range so that instruments like piccolo (which can't play C4) are measured
//! correctly.  Percussive instruments (xylophone, marimba, …) are handled by
//! measuring peak amplitude rather than RMS, so their short attack is captured
//! instead of being buried by the silent tail.
//!
//! Only SFZ files are measured; SF2 entries are skipped (gain = 0).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mapping_path: PathBuf = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../soundfonts/mapping.json"));

    // Optional second argument: directory to export labelled WAV samples to.
    let export_dir: Option<PathBuf> = args.get(2).map(PathBuf::from);

    let sfz_dir = mapping_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let mapping_text = std::fs::read_to_string(&mapping_path)
        .map_err(|e| anyhow::anyhow!("Cannot read {}: {e}", mapping_path.display()))?;

    let mapping: serde_json::Value = serde_json::from_str(&mapping_text)?;

    // ------------------------------------------------------------------
    // Collect every unique SFZ path referenced by the mapping.
    // SF2 files are skipped — their internal GM normalisation is handled
    // by FluidSynth.
    // ------------------------------------------------------------------
    let mut sfz_paths: HashSet<String> = HashSet::new();

    let add = |v: Option<&serde_json::Value>, set: &mut HashSet<String>| {
        if let Some(s) = v.and_then(|v| v.as_str()) {
            if !s.to_lowercase().ends_with(".sf2") {
                set.insert(s.to_string());
            }
        }
    };

    add(mapping.get("percussion"), &mut sfz_paths);
    add(mapping.get("fallback"), &mut sfz_paths);
    // Programs entries can be a plain string or a detail object with
    // "sfz", "staccato", "vibrato", and "overrides" fields.
    if let Some(programs) = mapping.get("programs").and_then(|v| v.as_object()) {
        for val in programs.values() {
            if val.is_string() {
                add(Some(val), &mut sfz_paths);
            } else if let Some(obj) = val.as_object() {
                add(obj.get("sfz"), &mut sfz_paths);
                add(obj.get("staccato"), &mut sfz_paths);
                add(obj.get("vibrato"), &mut sfz_paths);
                if let Some(overrides) = obj.get("overrides").and_then(|v| v.as_object()) {
                    for sfz_val in overrides.values() {
                        add(Some(sfz_val), &mut sfz_paths);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Locate sfizz_render — honour SFIZZ_BIN env var first, then PATH
    // ------------------------------------------------------------------
    let sfizz = find_sfizz().ok_or_else(|| {
        anyhow::anyhow!(
            "sfizz_render not found in PATH. \
             Install sfizz and ensure sfizz_render is on PATH (or set SFIZZ_BIN)."
        )
    })?;

    // Locate ffmpeg (needed only for corrected export; not fatal if absent).
    let ffmpeg = find_binary(&["ffmpeg", "ffmpeg.exe"]);

    println!("sfizz_render : {sfizz}");
    println!(
        "mapping      : {}",
        mapping_path.canonicalize().unwrap_or(mapping_path.clone()).display()
    );
    println!("sfz dir      : {}", sfz_dir.display());
    if let Some(dir) = &export_dir {
        println!("export dir   : {}", dir.display());
    }
    println!();

    // ------------------------------------------------------------------
    // Prepare a shared test MIDI file (C4, vel=80, 3 s, prog 0)
    // ------------------------------------------------------------------
    let tmp = std::env::temp_dir().join("normalize_mapping");
    std::fs::create_dir_all(&tmp)?;
    println!("Working directory: {}", tmp.display());
    println!();

    // ------------------------------------------------------------------
    // Render each SFZ and measure its peak level
    // ------------------------------------------------------------------
    let mut level_db: HashMap<String, f64> = HashMap::new();
    // Maps rel path → temp WAV path (for the corrected-export pass later)
    let mut wav_by_rel: HashMap<String, PathBuf> = HashMap::new();
    let mut sorted_paths: Vec<String> = sfz_paths.into_iter().collect();
    sorted_paths.sort();

    if let Some(dir) = &export_dir {
        std::fs::create_dir_all(dir)?;
    }

    println!("Measuring {} SFZ instrument(s)…", sorted_paths.len());
    println!("{:-<60}", "");

    for rel in &sorted_paths {
        let sfz_path = sfz_dir.join(rel);
        if !sfz_path.exists() {
            println!("  SKIP   {rel}  (file not found)");
            continue;
        }

        // Pick a note in this instrument's own SFZ key range.
        let (lo, hi) = sfz_key_range(&sfz_path).unwrap_or((60, 60));
        let note = (lo as u16 + hi as u16) / 2;  // midpoint, safe from overflow
        let note = note.min(127) as u8;

        // Build a safe filename, per-instrument MIDI and temp WAV path.
        let safe: String = rel
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '.' { c } else { '_' })
            .collect();
        let midi_path = tmp.join(format!("{safe}.mid"));
        write_test_midi(&midi_path, note)?;
        let wav_path = tmp.join(format!("{safe}.wav"));

        let result = std::process::Command::new(&sfizz)
            .arg("--sfz")
            .arg(&sfz_path)
            .arg("--midi")
            .arg(&midi_path)
            .arg("--wav")
            .arg(&wav_path)
            .arg("--samplerate")
            .arg("48000")
            .output();

        match result {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let msg = String::from_utf8_lossy(&o.stderr);
                let first = msg.trim().lines().next().unwrap_or("unknown error");
                println!("  FAIL   {rel}  — {first}");
                continue;
            }
            Err(e) => {
                println!("  FAIL   {rel}  — spawn error: {e}");
                continue;
            }
        }

        match measure_peak(&wav_path) {
            Ok(peak) if peak > 1e-5 => {
                let db = 20.0 * peak.log10();
                println!("  {:+7.2} dBFS  note={note:3}  {rel}", db);
                level_db.insert(rel.clone(), db);
                wav_by_rel.insert(rel.clone(), wav_path.clone());

                // Copy raw WAV to export dir immediately
                if let Some(dir) = &export_dir {
                    let dest = dir.join(format!("raw_{safe}.wav"));
                    let _ = std::fs::copy(&wav_path, &dest);
                }
            }
            Ok(_) => println!(
                "  SILENT {rel}  note={note:3}  (no output — wrong range? file corrupt?)"
            ),
            Err(e) => println!("  ERR    {rel}  — {e}"),
        }
    }

    if level_db.is_empty() {
        anyhow::bail!("No instruments could be measured. Check sfizz_render and your SFZ files.");
    }

    // ------------------------------------------------------------------
    // Compute target level = median of all measured levels, then gain
    // correction for each instrument.
    // ------------------------------------------------------------------
    let mut levels: Vec<f64> = level_db.values().cloned().collect();
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target_db = levels[levels.len() / 2];

    println!();
    println!("Target level (median): {target_db:+.2} dBFS");
    println!();
    println!("Gain corrections:");
    println!("{:-<60}", "");

    let mut gains_obj = serde_json::Map::new();
    let mut adjustments: Vec<(String, f64)> = level_db
        .iter()
        .map(|(k, v)| (k.clone(), target_db - v))
        .collect();
    adjustments.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // loudest cut first

    for (rel, gain) in &adjustments {
        println!("  {:+6.1} dB  {rel}", gain);
        let rounded = (gain * 10.0).round() / 10.0; // 1 decimal place
        gains_obj.insert(rel.clone(), serde_json::json!(rounded));
    }

    // ------------------------------------------------------------------
    // Write gains to gains.json (sibling of mapping.json)
    // ------------------------------------------------------------------
    let gains_path = sfz_dir.join("gains.json");
    let out = serde_json::to_string_pretty(&serde_json::Value::Object(gains_obj))?;
    std::fs::write(&gains_path, out)?;

    println!();
    println!(
        "Wrote {} gain entries to {}",
        adjustments.len(),
        gains_path.display()
    );

    // ------------------------------------------------------------------
    // Export gain-corrected WAVs so you can listen and verify
    // ------------------------------------------------------------------
    if let Some(dir) = &export_dir {
        if let Some(ff) = &ffmpeg {
            println!();
            println!("Writing gain-corrected WAVs to {} …", dir.display());
            for (rel, gain) in &adjustments {
                let Some(src_wav) = wav_by_rel.get(rel) else { continue };
                let safe: String = rel
                    .chars()
                    .map(|c| if c.is_alphanumeric() || c == '.' { c } else { '_' })
                    .collect();
                let dest = dir.join(format!("corrected_{safe}.wav"));
                let mut cmd = std::process::Command::new(ff);
                cmd.arg("-y").arg("-i").arg(src_wav);
                if gain.abs() > 0.05 {
                    cmd.arg("-af").arg(format!("volume={:.2}dB", gain));
                }
                cmd.arg("-c:a").arg("pcm_s16le").arg(&dest);
                match cmd.output() {
                    Ok(o) if o.status.success() => {
                        println!("  OK  corrected_{safe}.wav  ({:+.1} dB)", gain);
                    }
                    Ok(o) => {
                        let msg = String::from_utf8_lossy(&o.stderr);
                        let first = msg.trim().lines().next().unwrap_or("?");
                        println!("  ERR corrected_{safe}.wav  — {first}");
                    }
                    Err(e) => println!("  ERR corrected_{safe}.wav  — {e}"),
                }
            }
        } else {
            println!();
            println!(
                "Note: ffmpeg not found — skipping gain-corrected export. \
                 Raw WAVs are in {}",
                dir.display()
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test MIDI generation
// ---------------------------------------------------------------------------

/// Write a minimal Format-1 MIDI file: the given `note` at velocity 80 (mf)
/// for 3 seconds, 120 BPM, 480 ticks per quarter-note, GM program 0.
///
/// 3 s at 120 BPM = 6 quarter-notes = 2880 ticks.
/// VLQ(2880) = [0x96, 0x40]
fn write_test_midi(path: &Path, note: u8) -> anyhow::Result<()> {
    #[rustfmt::skip]
    let bytes: Vec<u8> = vec![
        // MThd
        0x4D, 0x54, 0x68, 0x64,
        0x00, 0x00, 0x00, 0x06, // chunk length = 6
        0x00, 0x01,             // format 1
        0x00, 0x02,             // 2 tracks
        0x01, 0xE0,             // 480 ticks / quarter-note

        // MTrk 0 — tempo track (11 bytes)
        0x4D, 0x54, 0x72, 0x6B,
        0x00, 0x00, 0x00, 0x0B, // chunk length = 11
        0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // Δ=0  set tempo 500000 µs (120 BPM)
        0x00, 0xFF, 0x2F, 0x00,                    // Δ=0  end of track

        // MTrk 1 — note track (16 bytes)
        // Δ=0    program change ch0 → prog 0
        // Δ=0    note on  ch0  `note` vel 80
        // Δ=2880 note off ch0  `note` vel 0
        // Δ=0    end of track
        0x4D, 0x54, 0x72, 0x6B,
        0x00, 0x00, 0x00, 0x10, // chunk length = 16
        0x00, 0xC0, 0x00,       // program change → prog 0
        0x00, 0x90, note, 0x50, // note on
        0x96, 0x40, 0x80, note, 0x00, // note off (after 2880 ticks)
        0x00, 0xFF, 0x2F, 0x00, // end of track
    ];

    std::fs::write(path, &bytes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Peak measurement
// ---------------------------------------------------------------------------

/// Parse a WAV file and return the peak amplitude (max |sample|) over the
/// entire signal.  This correctly handles percussive instruments (xylophone,
/// marimba, …) whose energy is concentrated in a brief attack; using the
/// middle-60% RMS window would report near-silence for such instruments.
/// Supports 16-bit PCM, 24-bit PCM, and 32-bit IEEE-float WAVs.
fn measure_peak(wav_path: &Path) -> anyhow::Result<f64> {
    let data = std::fs::read(wav_path)?;

    if data.len() < 44 {
        anyhow::bail!("file too small to be a valid WAV");
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        anyhow::bail!("not a RIFF/WAVE file");
    }

    let mut pos = 12usize;
    let mut audio_format: u16 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut data_offset: usize = 0;
    let mut data_size: usize = 0;

    // Walk RIFF chunks
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size =
            u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let cstart = pos + 8;

        if id == b"fmt " && size >= 16 {
            audio_format =
                u16::from_le_bytes(data[cstart..cstart + 2].try_into().unwrap());
            // channels at cstart+2
            // sample_rate at cstart+4
            // byte_rate at cstart+8
            // block_align at cstart+12
            bits_per_sample =
                u16::from_le_bytes(data[cstart + 14..cstart + 16].try_into().unwrap());
        } else if id == b"data" {
            data_offset = cstart;
            data_size = size;
            break;
        }

        // RIFF chunks are always word-aligned (pad byte if size is odd)
        pos = cstart + size + (size & 1);
    }

    if data_offset == 0 {
        anyhow::bail!("no 'data' chunk found in WAV");
    }

    let raw = &data[data_offset..(data_offset + data_size).min(data.len())];

    // Convert all samples to f64 in [-1.0, 1.0]
    let samples: Vec<f64> = match (audio_format, bits_per_sample) {
        (1, 16) => raw
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f64 / 32_768.0)
            .collect(),
        (1, 24) => raw
            .chunks_exact(3)
            .map(|b| {
                let raw_i = (b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16);
                // sign-extend 24-bit two's complement
                let v = if raw_i & 0x80_0000 != 0 {
                    raw_i | !0xFF_FFFF
                } else {
                    raw_i
                };
                v as f64 / 8_388_608.0
            })
            .collect(),
        (3, 32) => raw
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64)
            .collect(),
        _ => anyhow::bail!(
            "unsupported WAV format {audio_format} / {bits_per_sample}-bit"
        ),
    };

    if samples.is_empty() {
        return Ok(0.0);
    }

    // Peak amplitude over the entire signal — works for both sustained
    // instruments and percussive ones whose energy lives in a brief attack.
    let peak = samples.iter().map(|&s| s.abs()).fold(0.0f64, f64::max);
    Ok(peak)
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Locate sfizz_render: honour SFIZZ_BIN env var first, then probe PATH.
/// Mirrors the logic in `audio.rs` — only requires the process to spawn
/// successfully (exit code is irrelevant; sfizz_render --help exits non-zero).
fn find_sfizz() -> Option<String> {
    if let Ok(path) = std::env::var("SFIZZ_BIN") {
        let p = path.trim().to_owned();
        if !p.is_empty() {
            return Some(p);
        }
    }
    for candidate in ["sfizz_render", "sfizz_render.exe"] {
        if std::process::Command::new(candidate)
            .arg("--help")
            .output()
            .is_ok()
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

fn find_binary(candidates: &[&str]) -> Option<String> {
    for &name in candidates {
        if std::process::Command::new(name)
            .arg("--help")
            .output()
            .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
            .unwrap_or(false)
        {
            return Some(name.to_owned());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// SFZ key-range detection
// ---------------------------------------------------------------------------

/// Scan an SFZ file for `lokey`, `hikey`, and `key` opcodes and return the
/// overall `(lo, hi)` MIDI note range.  Returns `None` if no key opcodes are
/// found (caller falls back to C4 = 60).
///
/// Handles both numeric values (`lokey=60`) and note names (`lokey=c4`,
/// `lokey=d#5`, `lokey=eb3`).
fn sfz_key_range(sfz_path: &Path) -> Option<(u8, u8)> {
    let text = std::fs::read_to_string(sfz_path).ok()?;
    let mut lo: u8 = 127;
    let mut hi: u8 = 0;
    let mut found = false;

    for line in text.lines() {
        // Strip line comments
        let line = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        // Each opcode is a `key=value` token; split on whitespace
        for token in line.split_whitespace() {
            let Some((k, v)) = token.split_once('=') else { continue };
            let midi = match note_to_midi(v.trim()) {
                Some(n) => n,
                None => continue,
            };
            match k.trim().to_ascii_lowercase().as_str() {
                "lokey" => { lo = lo.min(midi); found = true; }
                "hikey" => { hi = hi.max(midi); found = true; }
                // `key=N` is shorthand for `lokey=N hikey=N`
                "key"   => { lo = lo.min(midi); hi = hi.max(midi); found = true; }
                _ => {}
            }
        }
    }

    if found {
        // If only lokey was ever set, hi stays 0 < lo — swap so range is valid.
        let (a, b) = (lo.min(hi), lo.max(hi));
        Some((a, b))
    } else {
        None
    }
}

/// Convert an SFZ note value to a MIDI note number.
/// Accepts integers ("60") and note names ("c4", "d#5", "eb3", "C-1").
/// In SFZ notation: C-1 = 0, C0 = 12, C4 = 60, C8 = 108.
fn note_to_midi(s: &str) -> Option<u8> {
    // Try plain integer first
    if let Ok(n) = s.parse::<u8>() {
        return Some(n);
    }
    let s = s.to_ascii_lowercase();
    let mut chars = s.chars().peekable();
    let base: i32 = match chars.next()? {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _   => return None,
    };
    let accidental: i32 = match chars.peek() {
        Some('#') => { chars.next(); 1 }
        Some('b') => { chars.next(); -1 }
        _ => 0,
    };
    let octave_str: String = chars.collect();
    let octave: i32 = octave_str.parse().ok()?;
    let midi = (octave + 1) * 12 + base + accidental;
    if (0..=127).contains(&midi) { Some(midi as u8) } else { None }
}
