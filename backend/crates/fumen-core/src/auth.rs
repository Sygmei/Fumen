use anyhow::Result;
use chrono::{Duration, Utc};
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::db::DbPool;
use crate::models::{NewUser, NewUserLoginLink, UserRecord};
use crate::schema::{user_login_links, users};
use crate::{format_timestamp, generate_auth_token, normalize_username, utc_now_string};

const SUPERADMIN_ROLE: &str = "superadmin";

#[derive(Clone, Debug)]
pub struct CreatedLoginLink {
    pub connection_url: String,
    pub expires_at: String,
}

pub async fn ensure_superadmin_user(db: &DbPool, config: &AppConfig) -> Result<UserRecord> {
    let mut conn = db.get().await?;

    if let Some(existing) = users::table
        .filter(
            users::role
                .eq(SUPERADMIN_ROLE)
                .or(users::is_superadmin.eq(true)),
        )
        .select(UserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?
    {
        return Ok(existing);
    }

    let base_username = normalize_username(&config.superadmin_username)?;
    let mut username = base_username.clone();

    while users::table
        .filter(users::username.eq(&username))
        .select(users::id)
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

    let record = UserRecord {
        id: Uuid::new_v4().to_string(),
        username,
        created_at: utc_now_string(),
        is_superadmin: true,
        role: SUPERADMIN_ROLE.to_owned(),
        display_name: None,
        avatar_image_key: None,
        created_by_user_id: None,
    };

    diesel::insert_into(users::table)
        .values(&NewUser {
            id: &record.id,
            username: &record.username,
            created_at: &record.created_at,
            is_superadmin: record.is_superadmin,
            role: &record.role,
            display_name: record.display_name.as_deref(),
            avatar_image_key: record.avatar_image_key.as_deref(),
            created_by_user_id: record.created_by_user_id.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    Ok(record)
}

pub async fn create_login_link(
    db: &DbPool,
    config: &AppConfig,
    user_id: &str,
    ttl_minutes: i64,
) -> Result<CreatedLoginLink> {
    let now = Utc::now();
    let created_at = format_timestamp(now);
    let expires_at = format_timestamp(now + Duration::minutes(ttl_minutes));
    let token = generate_auth_token(48);

    let mut conn = db.get().await?;
    let login_link_id = Uuid::new_v4().to_string();
    diesel::insert_into(user_login_links::table)
        .values(&NewUserLoginLink {
            id: &login_link_id,
            user_id,
            token: &token,
            created_at: &created_at,
            expires_at: &expires_at,
            consumed_at: None,
        })
        .execute(&mut conn)
        .await?;

    Ok(CreatedLoginLink {
        connection_url: config.connection_url_for(&token),
        expires_at,
    })
}
