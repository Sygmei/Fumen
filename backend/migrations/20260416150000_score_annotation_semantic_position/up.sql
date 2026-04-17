DROP TABLE IF EXISTS score_annotations;

CREATE TABLE score_annotations (
    id TEXT PRIMARY KEY,
    music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bar_number BIGINT NOT NULL,
    beat_number BIGINT NOT NULL,
    instrument TEXT NOT NULL,
    comment TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX score_annotations_music_idx
    ON score_annotations (music_id, bar_number, beat_number, created_at);

CREATE INDEX score_annotations_user_idx
    ON score_annotations (user_id, music_id, created_at);