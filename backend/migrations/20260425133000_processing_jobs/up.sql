CREATE TABLE processing_jobs (
    music_id TEXT PRIMARY KEY REFERENCES musics(id) ON DELETE CASCADE,
    source_object_key TEXT NOT NULL,
    source_filename TEXT NOT NULL,
    quality_profile TEXT NOT NULL,
    status TEXT NOT NULL,
    current_step TEXT NOT NULL,
    attempt BIGINT NOT NULL DEFAULT 1,
    max_attempts BIGINT NOT NULL DEFAULT 25,
    worker_id TEXT,
    lease_expires_at TEXT,
    heartbeat_at TEXT,
    queued_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    error_message TEXT
);

CREATE INDEX processing_jobs_status_queued_at_idx
    ON processing_jobs (status, queued_at);

CREATE INDEX processing_jobs_lease_expires_at_idx
    ON processing_jobs (lease_expires_at);
