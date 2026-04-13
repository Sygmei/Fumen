#[path = "../config.rs"]
mod config;
#[allow(dead_code)]
#[path = "../db.rs"]
mod db;
#[allow(dead_code)]
#[path = "../models.rs"]
mod models;
#[path = "../schema.rs"]
mod schema;

use anyhow::{Context, Result, anyhow};
use chrono::{Duration, SecondsFormat, Utc};
use config::AppConfig;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use models::{NewUser, NewUserLoginLink};
use qrcode::{QrCode, render::unicode};
use rand::{Rng, distr::Alphanumeric};
use uuid::Uuid;

const LOGIN_LINK_TTL_MINUTES: i64 = 5;

struct CliLoginLink {
    connection_url: String,
    expires_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = schema::users)]
struct CliUserRecord {
    id: String,
    username: String,
    created_at: String,
    is_superadmin: bool,
    role: String,
    display_name: Option<String>,
    avatar_image_key: Option<String>,
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
    db::run_migrations(&config.database_url_admin).await?;
    let db_admin = db::open_database_pool(&config.database_url_admin, 1, "admin").await?;
    let superadmin = ensure_superadmin_user(&db_admin, &config).await?;
    let db_rw = db::open_database_pool(&config.database_url, 5, "read-write").await?;

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

async fn ensure_superadmin_user(db: &db::DbPool, config: &AppConfig) -> Result<CliUserRecord> {
    let mut conn = db.get().await?;
    use schema::users::dsl as users_dsl;

    if let Some(existing) = users_dsl::users
        .filter(
            users_dsl::role
                .eq("superadmin")
                .or(users_dsl::is_superadmin.eq(true)),
        )
        .select(CliUserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?
    {
        return Ok(existing);
    }

    let base_username = normalize_username(&config.superadmin_username)?;
    let mut username = base_username.clone();

    while users_dsl::users
        .filter(users_dsl::username.eq(&username))
        .select(users_dsl::id)
        .first::<String>(&mut conn)
        .await
        .optional()?
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
        display_name: None,
        avatar_image_key: None,
        created_by_user_id: None,
    };

    diesel::insert_into(users_dsl::users)
        .values(&NewUser {
            id: &record.id,
            username: &record.username,
            created_at: &record.created_at,
            is_superadmin: record.is_superadmin,
            role: &record.role,
            display_name: None,
            avatar_image_key: None,
            created_by_user_id: None,
        })
        .execute(&mut conn)
        .await?;

    Ok(record)
}

async fn find_user_by_lookup(db: &db::DbPool, lookup: &str) -> Result<Option<CliUserRecord>> {
    let mut conn = db.get().await?;
    use schema::users::dsl as users_dsl;

    if let Some(found) = users_dsl::users
        .filter(users_dsl::username.eq(lookup))
        .select(CliUserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?
    {
        return Ok(Some(found));
    }

    Ok(users_dsl::users
        .filter(users_dsl::id.eq(lookup))
        .select(CliUserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

async fn create_login_link(
    db: &db::DbPool,
    config: &AppConfig,
    user_id: &str,
) -> Result<CliLoginLink> {
    let now = Utc::now();
    let created_at = format_timestamp(now);
    let expires_at = format_timestamp(now + Duration::minutes(LOGIN_LINK_TTL_MINUTES));
    let token = generate_auth_token(48);

    let mut conn = db.get().await?;
    diesel::insert_into(schema::user_login_links::table)
        .values(&NewUserLoginLink {
            id: &Uuid::new_v4().to_string(),
            user_id,
            token: &token,
            created_at: &created_at,
            expires_at: &expires_at,
            consumed_at: None,
        })
        .execute(&mut conn)
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
