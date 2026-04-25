use anyhow::Result;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::db::DbPool;
use crate::models::StemRecord;
use crate::schema::stems;

pub async fn find_public_stems(
    db_primary: &DbPool,
    db_fallback: &DbPool,
    music_id: &str,
) -> Result<Vec<StemRecord>> {
    let mut primary = db_primary.get().await?;
    let stems = stems::table
        .filter(stems::music_id.eq(music_id))
        .order(stems::track_index.asc())
        .select(StemRecord::as_select())
        .load(&mut primary)
        .await?;

    if !stems.is_empty() {
        return Ok(stems);
    }

    let mut fallback = db_fallback.get().await?;
    Ok(stems::table
        .filter(stems::music_id.eq(music_id))
        .order(stems::track_index.asc())
        .select(StemRecord::as_select())
        .load(&mut fallback)
        .await?)
}

pub fn processing_log_key(music_id: &str) -> String {
    format!("scores/{music_id}/processing.log")
}
