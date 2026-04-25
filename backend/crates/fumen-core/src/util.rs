use anyhow::{Result, anyhow};
use chrono::{SecondsFormat, Utc};
use rand::{Rng, distr::Alphanumeric};

pub fn utc_now_string() -> String {
    format_timestamp(Utc::now())
}

pub fn format_timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn generate_auth_token(length: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub fn normalize_username(raw: &str) -> Result<String> {
    let value = raw.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len()) {
        return Err(anyhow!("Usernames must be between 3 and 32 characters"));
    }

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(anyhow!(
            "Usernames can only contain letters, numbers, hyphens, and underscores"
        ));
    }

    Ok(value)
}

pub fn sanitize_filename(filename: &str) -> String {
    let mut sanitized = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || character == '.'
                || character == '-'
                || character == '_'
            {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        sanitized = "score.mscz".to_owned();
    }

    sanitized
}
