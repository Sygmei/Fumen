CREATE TABLE IF NOT EXISTS directories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    is_superadmin BOOLEAN NOT NULL DEFAULT FALSE,
    role TEXT NOT NULL DEFAULT 'user',
    display_name TEXT,
    avatar_image_key TEXT,
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS ensembles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS musics (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    subtitle TEXT,
    icon TEXT,
    icon_image_key TEXT,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    object_key TEXT NOT NULL,
    audio_object_key TEXT,
    audio_status TEXT NOT NULL DEFAULT 'unavailable',
    audio_error TEXT,
    midi_object_key TEXT,
    midi_status TEXT NOT NULL DEFAULT 'unavailable',
    midi_error TEXT,
    musicxml_object_key TEXT,
    musicxml_status TEXT NOT NULL DEFAULT 'unavailable',
    musicxml_error TEXT,
    stems_status TEXT NOT NULL DEFAULT 'unavailable',
    stems_error TEXT,
    public_token TEXT NOT NULL UNIQUE,
    public_id TEXT UNIQUE,
    quality_profile TEXT NOT NULL DEFAULT 'standard',
    created_at TEXT NOT NULL,
    directory_id TEXT NOT NULL DEFAULT '',
    owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS stems (
    id BIGSERIAL PRIMARY KEY,
    music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
    track_index BIGINT NOT NULL,
    track_name TEXT NOT NULL,
    instrument_name TEXT NOT NULL,
    storage_key TEXT NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    drum_map_json TEXT
);

CREATE TABLE IF NOT EXISTS user_ensemble_memberships (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ensemble_id TEXT NOT NULL REFERENCES ensembles(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'user',
    PRIMARY KEY (user_id, ensemble_id)
);

CREATE TABLE IF NOT EXISTS directory_ensemble_permissions (
    directory_id TEXT NOT NULL REFERENCES directories(id) ON DELETE CASCADE,
    ensemble_id TEXT NOT NULL REFERENCES ensembles(id) ON DELETE CASCADE,
    PRIMARY KEY (directory_id, ensemble_id)
);

CREATE TABLE IF NOT EXISTS music_ensemble_links (
    music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
    ensemble_id TEXT NOT NULL REFERENCES ensembles(id) ON DELETE CASCADE,
    PRIMARY KEY (music_id, ensemble_id)
);

CREATE TABLE IF NOT EXISTS user_login_links (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    consumed_at TEXT
);

CREATE TABLE IF NOT EXISTS user_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_token TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT
);

CREATE TABLE IF NOT EXISTS user_music_track_playtime (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
    track_index BIGINT NOT NULL,
    total_seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, music_id, track_index)
);

ALTER TABLE musics ADD COLUMN IF NOT EXISTS icon TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS subtitle TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS icon_image_key TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS audio_object_key TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS audio_status TEXT NOT NULL DEFAULT 'unavailable';
ALTER TABLE musics ADD COLUMN IF NOT EXISTS audio_error TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS midi_object_key TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS midi_status TEXT NOT NULL DEFAULT 'unavailable';
ALTER TABLE musics ADD COLUMN IF NOT EXISTS midi_error TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS musicxml_object_key TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS musicxml_status TEXT NOT NULL DEFAULT 'unavailable';
ALTER TABLE musics ADD COLUMN IF NOT EXISTS musicxml_error TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS stems_status TEXT NOT NULL DEFAULT 'unavailable';
ALTER TABLE musics ADD COLUMN IF NOT EXISTS stems_error TEXT;
ALTER TABLE musics ADD COLUMN IF NOT EXISTS quality_profile TEXT NOT NULL DEFAULT 'standard';
ALTER TABLE musics ADD COLUMN IF NOT EXISTS directory_id TEXT NOT NULL DEFAULT '';
ALTER TABLE musics ADD COLUMN IF NOT EXISTS owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE stems ADD COLUMN IF NOT EXISTS size_bytes BIGINT NOT NULL DEFAULT 0;
ALTER TABLE stems ADD COLUMN IF NOT EXISTS drum_map_json TEXT;

ALTER TABLE users ADD COLUMN IF NOT EXISTS is_superadmin BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';
ALTER TABLE users ADD COLUMN IF NOT EXISTS display_name TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_image_key TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ensembles ADD COLUMN IF NOT EXISTS created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE user_ensemble_memberships ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';
ALTER TABLE user_sessions ALTER COLUMN expires_at DROP NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS users_single_superadmin_idx
    ON users (is_superadmin)
    WHERE is_superadmin = TRUE;

CREATE INDEX IF NOT EXISTS user_music_track_playtime_music_idx
    ON user_music_track_playtime (music_id, user_id);

UPDATE users
SET role = 'superadmin'
WHERE is_superadmin = TRUE;

UPDATE users
SET role = 'manager'
WHERE role = 'user'
  AND is_superadmin = FALSE
  AND EXISTS (
      SELECT 1
      FROM user_ensemble_memberships uem
      WHERE uem.user_id = users.id
        AND uem.role = 'admin'
  );

UPDATE users
SET role = 'user'
WHERE role IS NULL
   OR role NOT IN ('superadmin', 'admin', 'manager', 'editor', 'user');

UPDATE users
SET is_superadmin = (role = 'superadmin');

UPDATE user_ensemble_memberships
SET role = 'manager'
WHERE role = 'admin';

UPDATE user_ensemble_memberships
SET role = 'user'
WHERE role IS NULL
   OR role NOT IN ('user', 'manager', 'editor');

INSERT INTO music_ensemble_links (music_id, ensemble_id)
SELECT m.id, dep.ensemble_id
FROM musics m
JOIN directory_ensemble_permissions dep ON dep.directory_id = m.directory_id
ON CONFLICT DO NOTHING;
