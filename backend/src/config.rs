use anyhow::{Result, anyhow};
use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AppConfig {
    pub bind_address: String,
    pub app_base_url: String,
    /// `None` means "allow any origin" (no `CORS_ALLOWED_ORIGINS` env var was set).
    pub cors_allowed_origins: Option<Vec<String>>,
    pub database_url: String,
    pub database_url_admin: String,
    pub database_url_read_only: String,
    pub superadmin_username: String,
    pub jwt_secret: String,
    pub storage: StorageConfig,
    pub musescore_bin: Option<String>,
    pub musescore_docker_image: Option<String>,
    pub musescore_qt_platform: Option<String>,
    pub docker_bin: String,
    pub processor_poll_interval_ms: u64,
    pub processor_lease_seconds: i64,
    pub processor_heartbeat_interval_ms: u64,
    pub processor_max_parallel_stem_renders: usize,
    pub processor_worker_id: Option<String>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum StorageConfig {
    Local { root: PathBuf },
    S3(S3Config),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub force_path_style: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
        let app_base_url =
            env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:5173".to_owned());
        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|origins| !origins.is_empty());
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow!("Set DATABASE_URL to a PostgreSQL connection string."))?;
        let database_url_admin =
            env::var("DATABASE_URL_ADMIN").unwrap_or_else(|_| database_url.clone());
        let database_url_read_only =
            env::var("DATABASE_URL_READ_ONLY").unwrap_or_else(|_| database_url.clone());
        let superadmin_username =
            env::var("SUPERADMIN_USERNAME").unwrap_or_else(|_| "superadmin".to_owned());

        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| anyhow!("Set JWT_SECRET to a secure random string for signing JWTs."))?;
        let local_storage_path = PathBuf::from(
            env::var("LOCAL_STORAGE_PATH").unwrap_or_else(|_| "./data/storage".to_owned()),
        );

        let s3_bucket = env::var("S3_BUCKET")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_access_key_id = env::var("S3_ACCESS_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_secret_access_key = env::var("S3_SECRET_ACCESS_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let storage = match (s3_bucket, s3_access_key_id, s3_secret_access_key) {
            (Some(bucket), Some(access_key_id), Some(secret_access_key)) => {
                StorageConfig::S3(S3Config {
                    bucket,
                    region: env::var("S3_REGION").unwrap_or_else(|_| "eu-west-3".to_owned()),
                    endpoint: env::var("S3_ENDPOINT")
                        .ok()
                        .filter(|value| !value.trim().is_empty()),
                    access_key_id,
                    secret_access_key,
                    force_path_style: env::var("S3_FORCE_PATH_STYLE")
                        .map(|value| {
                            matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES")
                        })
                        .unwrap_or(false),
                })
            }
            (None, None, None) => StorageConfig::Local {
                root: local_storage_path,
            },
            _ => {
                return Err(anyhow!(
                    "To enable S3 storage, set S3_BUCKET, S3_ACCESS_KEY_ID, and S3_SECRET_ACCESS_KEY. Otherwise leave them unset to use local storage."
                ));
            }
        };

        let musescore_bin = env::var("MUSESCORE_BIN")
            .ok()
            .filter(|value| !value.trim().is_empty());

        let musescore_docker_image = env::var("MUSESCORE_DOCKER_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty());

        let musescore_qt_platform = env::var("MUSESCORE_QT_PLATFORM")
            .ok()
            .filter(|value| !value.trim().is_empty());

        let docker_bin = env::var("DOCKER_BIN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "docker".to_owned());
        let processor_poll_interval_ms = env::var("PROCESSOR_POLL_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(10_000);
        let processor_lease_seconds = env::var("PROCESSOR_LEASE_SECONDS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(600);
        let processor_heartbeat_interval_ms = env::var("PROCESSOR_HEARTBEAT_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(30_000);
        let processor_max_parallel_stem_renders = env::var("PROCESSOR_MAX_PARALLEL_STEM_RENDERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or_else(default_parallelism);
        let processor_worker_id = env::var("PROCESSOR_WORKER_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Ok(Self {
            bind_address,
            app_base_url,
            cors_allowed_origins,
            database_url,
            database_url_admin,
            database_url_read_only,
            superadmin_username,
            jwt_secret,
            storage,
            musescore_bin,
            musescore_docker_image,
            musescore_qt_platform,
            docker_bin,
            processor_poll_interval_ms,
            processor_lease_seconds,
            processor_heartbeat_interval_ms,
            processor_max_parallel_stem_renders,
            processor_worker_id,
        })
    }

    #[allow(dead_code)]
    pub fn public_url_for(&self, access_key: &str) -> String {
        format!(
            "{}/listen/{}",
            self.app_base_url.trim_end_matches('/'),
            access_key
        )
    }

    pub fn connection_url_for(&self, token: &str) -> String {
        format!(
            "{}/connect/{}",
            self.app_base_url.trim_end_matches('/'),
            token
        )
    }
}

fn default_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
}
