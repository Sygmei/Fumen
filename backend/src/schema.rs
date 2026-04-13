diesel::table! {
    directories (id) {
        id -> Text,
        name -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    ensembles (id) {
        id -> Text,
        name -> Text,
        created_at -> Text,
        created_by_user_id -> Nullable<Text>,
    }
}

diesel::table! {
    musics (id) {
        id -> Text,
        title -> Text,
        icon -> Nullable<Text>,
        icon_image_key -> Nullable<Text>,
        filename -> Text,
        content_type -> Text,
        object_key -> Text,
        audio_object_key -> Nullable<Text>,
        audio_status -> Text,
        audio_error -> Nullable<Text>,
        midi_object_key -> Nullable<Text>,
        midi_status -> Text,
        midi_error -> Nullable<Text>,
        musicxml_object_key -> Nullable<Text>,
        musicxml_status -> Text,
        musicxml_error -> Nullable<Text>,
        stems_status -> Text,
        stems_error -> Nullable<Text>,
        public_token -> Text,
        public_id -> Nullable<Text>,
        quality_profile -> Text,
        created_at -> Text,
        directory_id -> Text,
        owner_user_id -> Nullable<Text>,
    }
}

diesel::table! {
    music_ensemble_links (music_id, ensemble_id) {
        music_id -> Text,
        ensemble_id -> Text,
    }
}

diesel::table! {
    stems (id) {
        id -> BigInt,
        music_id -> Text,
        track_index -> BigInt,
        track_name -> Text,
        instrument_name -> Text,
        storage_key -> Text,
        size_bytes -> BigInt,
        drum_map_json -> Nullable<Text>,
    }
}

diesel::table! {
    user_ensemble_memberships (user_id, ensemble_id) {
        user_id -> Text,
        ensemble_id -> Text,
        role -> Text,
    }
}

diesel::table! {
    user_login_links (id) {
        id -> Text,
        user_id -> Text,
        token -> Text,
        created_at -> Text,
        expires_at -> Text,
        consumed_at -> Nullable<Text>,
    }
}

diesel::table! {
    user_music_track_playtime (user_id, music_id, track_index) {
        user_id -> Text,
        music_id -> Text,
        track_index -> BigInt,
        total_seconds -> Double,
        updated_at -> Text,
    }
}

diesel::table! {
    user_sessions (id) {
        id -> Text,
        user_id -> Text,
        session_token -> Text,
        created_at -> Text,
        expires_at -> Nullable<Text>,
    }
}

diesel::table! {
    users (id) {
        id -> Text,
        username -> Text,
        created_at -> Text,
        is_superadmin -> Bool,
        role -> Text,
        display_name -> Nullable<Text>,
        avatar_image_key -> Nullable<Text>,
        created_by_user_id -> Nullable<Text>,
    }
}

diesel::joinable!(music_ensemble_links -> ensembles (ensemble_id));
diesel::joinable!(music_ensemble_links -> musics (music_id));
diesel::joinable!(stems -> musics (music_id));
diesel::joinable!(user_ensemble_memberships -> ensembles (ensemble_id));
diesel::joinable!(user_ensemble_memberships -> users (user_id));
diesel::joinable!(user_login_links -> users (user_id));
diesel::joinable!(user_music_track_playtime -> musics (music_id));
diesel::joinable!(user_music_track_playtime -> users (user_id));
diesel::joinable!(user_sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    directories,
    ensembles,
    musics,
    music_ensemble_links,
    stems,
    user_ensemble_memberships,
    user_login_links,
    user_music_track_playtime,
    user_sessions,
    users,
);
