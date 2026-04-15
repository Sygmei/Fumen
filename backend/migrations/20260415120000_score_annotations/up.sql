CREATE TABLE IF NOT EXISTS score_annotations (
    id TEXT PRIMARY KEY,
    music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    step_index BIGINT NOT NULL,
    seconds DOUBLE PRECISION NOT NULL,
    comment TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS score_annotations_music_idx
    ON score_annotations (music_id, step_index, created_at);

CREATE INDEX IF NOT EXISTS score_annotations_user_idx
    ON score_annotations (user_id, music_id, created_at);