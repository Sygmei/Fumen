use anyhow::{Context, Result, anyhow};
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use fumen_core::{
    auth::{create_login_link, ensure_superadmin_user},
    config::AppConfig,
    db,
    models::UserRecord,
    schema,
};
use qrcode::{QrCode, render::unicode};

const LOGIN_LINK_TTL_MINUTES: i64 = 5;

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
    let user_lookup = std::env::args().nth(1).ok_or_else(|| {
        anyhow!(
            "usage: cargo run --package fumen-backend --bin login-link -- <username-or-user-id>"
        )
    })?;

    let config = AppConfig::from_env()?;
    db::run_migrations(&config.database_url_admin).await?;
    let db_admin = db::open_database_pool(&config.database_url_admin, 1, "admin").await?;
    let superadmin = ensure_superadmin_user(&db_admin, &config).await?;
    let db_rw = db::open_database_pool(&config.database_url, 5, "read-write").await?;

    let user = find_user_by_lookup(&db_rw, &user_lookup)
        .await?
        .ok_or_else(|| anyhow!("No user found for '{user_lookup}'"))?;
    let link = create_login_link(&db_rw, &config, &user.id, LOGIN_LINK_TTL_MINUTES).await?;

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

async fn find_user_by_lookup(db: &db::DbPool, lookup: &str) -> Result<Option<UserRecord>> {
    let mut conn = db.get().await?;
    use schema::users::dsl as users_dsl;

    if let Some(found) = users_dsl::users
        .filter(users_dsl::username.eq(lookup))
        .select(UserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?
    {
        return Ok(Some(found));
    }

    Ok(users_dsl::users
        .filter(users_dsl::id.eq(lookup))
        .select(UserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

fn print_cli_login_link(link: &fumen_core::auth::CreatedLoginLink) {
    let code =
        QrCode::new(link.connection_url.as_bytes()).expect("connection URL should be valid QR");
    let rendered = code.render::<unicode::Dense1x2>().quiet_zone(false).build();

    println!("{rendered}");
    println!();
    println!("Connection link: {}", link.connection_url);
    println!("Expires at: {}", link.expires_at);
}
