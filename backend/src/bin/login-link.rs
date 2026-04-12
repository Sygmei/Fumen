#[path = "../config.rs"]
mod config;

use anyhow::{Context, Result, anyhow};
use chrono::{Duration, SecondsFormat, Utc};
use config::AppConfig;
use qrcode::{QrCode, render::unicode};
use rand::{Rng, distr::Alphanumeric};
use sqlx::{
    FromRow, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;
use uuid::Uuid;

const LOGIN_LINK_TTL_MINUTES: i64 = 5;

struct CliLoginLink {
    connection_url: String,
    expires_at: String,
}

#[derive(Debug, FromRow)]
struct CliUserRecord {
    id: String,
    username: String,
    created_at: String,
    is_superadmin: bool,
    role: String,
    created_by_user_id: Option<String>,
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime")?;
    let result = runtime.block_on(async_main());
    drop(runtime);
    result
}

async fn async_main() -> Result<()> {
    let user_lookup = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: cargo run --bin login-link -- <username-or-user-id>"))?;

    let config = AppConfig::from_env()?;
    let db_admin = open_database_pool(&config.database_url_admin, 1, "admin").await?;
    ensure_cli_schema(&db_admin).await?;
    let superadmin = ensure_superadmin_user(&db_admin, &config).await?;
    let db_rw = open_database_pool(&config.database_url, 5, "read-write").await?;

    let user = find_user_by_lookup(&db_rw, &user_lookup)
        .await?
        .ok_or_else(|| anyhow!("No user found for '{user_lookup}'"))?;
    let link = create_login_link(&db_rw, &config, &user.id).await?;

    println!("User: {} ({})", user.username, user.id);
    if user.is_superadmin {
        println!("Role: superadmin");
    }
    if user.username == superadmin.username {
        println!("Seeded superadmin account confirmed.");
    }
    println!();
    print_cli_login_link(&link);
    Ok(())
}

async fn open_database_pool(url: &str, max_connections: u32, role: &str) -> Result<PgPool> {
    let options = PgConnectOptions::from_str(url)
        .with_context(|| format!("invalid PostgreSQL connection string for {role} pool"))?
        .statement_cache_capacity(0);

    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?)
}

async fn ensure_cli_schema(db: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            is_superadmin BOOLEAN NOT NULL DEFAULT FALSE,
            role TEXT NOT NULL DEFAULT 'user',
            created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_login_links (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            consumed_at TEXT
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        "ALTER TABLE users ADD COLUMN IF NOT EXISTS is_superadmin BOOLEAN NOT NULL DEFAULT FALSE",
    )
    .execute(db)
    .await?;
    sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user'")
        .execute(db)
        .await?;
    sqlx::query(
        "ALTER TABLE users ADD COLUMN IF NOT EXISTS created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL",
    )
    .execute(db)
    .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS users_single_superadmin_idx ON users (is_superadmin) WHERE is_superadmin = TRUE",
    )
    .execute(db)
    .await?;
    sqlx::query("UPDATE users SET role = 'superadmin' WHERE is_superadmin = TRUE")
        .execute(db)
        .await?;

    Ok(())
}

async fn ensure_superadmin_user(db: &PgPool, config: &AppConfig) -> Result<CliUserRecord> {
    if let Some(existing) = sqlx::query_as::<_, CliUserRecord>(
        "SELECT id, username, created_at, is_superadmin, role, created_by_user_id FROM users WHERE role = 'superadmin' OR is_superadmin = TRUE LIMIT 1",
    )
    .fetch_optional(db)
    .await?
    {
        return Ok(existing);
    }

    let base_username = normalize_username(&config.superadmin_username)?;
    let mut username = base_username.clone();

    while sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
        .bind(&username)
        .fetch_optional(db)
        .await?
        .is_some()
    {
        username = format!(
            "{base_username}-{}",
            generate_auth_token(6).to_ascii_lowercase()
        );
    }

    let record = CliUserRecord {
        id: Uuid::new_v4().to_string(),
        username,
        created_at: utc_now_string(),
        is_superadmin: true,
        role: "superadmin".to_owned(),
        created_by_user_id: None,
    };

    sqlx::query(
        "INSERT INTO users (id, username, created_at, is_superadmin, role, created_by_user_id) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&record.id)
    .bind(&record.username)
    .bind(&record.created_at)
    .bind(record.is_superadmin)
    .bind(&record.role)
    .bind(&record.created_by_user_id)
    .execute(db)
    .await?;

    Ok(record)
}

async fn find_user_by_lookup(db: &PgPool, lookup: &str) -> Result<Option<CliUserRecord>> {
    Ok(sqlx::query_as::<_, CliUserRecord>(
        r#"
        SELECT id, username, created_at, is_superadmin, role, created_by_user_id
        FROM users
        WHERE username = $1 OR id = $1
        ORDER BY CASE WHEN username = $1 THEN 0 ELSE 1 END
        LIMIT 1
        "#,
    )
    .bind(lookup)
    .fetch_optional(db)
    .await?)
}

async fn create_login_link(db: &PgPool, config: &AppConfig, user_id: &str) -> Result<CliLoginLink> {
    let now = Utc::now();
    let created_at = format_timestamp(now);
    let expires_at = format_timestamp(now + Duration::minutes(LOGIN_LINK_TTL_MINUTES));
    let token = generate_auth_token(48);

    sqlx::query(
        r#"
        INSERT INTO user_login_links (id, user_id, token, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(&token)
    .bind(&created_at)
    .bind(&expires_at)
    .execute(db)
    .await?;

    Ok(CliLoginLink {
        connection_url: config.connection_url_for(&token),
        expires_at,
    })
}

fn print_cli_login_link(link: &CliLoginLink) {
    let code =
        QrCode::new(link.connection_url.as_bytes()).expect("connection URL should be valid QR");
    let rendered = code.render::<unicode::Dense1x2>().quiet_zone(false).build();

    println!("{rendered}");
    println!();
    println!("Connection link: {}", link.connection_url);
    println!("Expires at: {}", link.expires_at);
}

fn normalize_username(raw: &str) -> Result<String> {
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

fn generate_auth_token(length: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn utc_now_string() -> String {
    format_timestamp(Utc::now())
}

fn format_timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}
