use crate::config::{AppConfig, S3Config, StorageConfig};
use anyhow::Result;
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    Client,
    config::{BehaviorVersion, Region, StalledStreamProtectionConfig},
    primitives::ByteStream,
    types::ObjectCannedAcl,
};
use bytes::Bytes;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Clone)]
pub struct Storage {
    backend: StorageBackend,
}

#[derive(Clone)]
enum StorageBackend {
    Local {
        root: PathBuf,
    },
    S3 {
        bucket: String,
        client: Client,
        public_base_url: String,
    },
}

impl Storage {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        let backend = match &config.storage {
            StorageConfig::Local { root } => {
                fs::create_dir_all(root).await?;
                StorageBackend::Local { root: root.clone() }
            }
            StorageConfig::S3(s3) => StorageBackend::S3 {
                bucket: s3.bucket.clone(),
                client: build_s3_client(s3),
                public_base_url: build_s3_public_base_url(s3),
            },
        };

        Ok(Self { backend })
    }

    pub async fn upload_bytes(&self, key: &str, bytes: Bytes, content_type: &str) -> Result<()> {
        self.upload_bytes_with_encoding(key, bytes, content_type, None)
            .await
    }

    pub async fn upload_bytes_quiet(
        &self,
        key: &str,
        bytes: Bytes,
        content_type: &str,
    ) -> Result<()> {
        self.upload_bytes_with_encoding_internal(key, bytes, content_type, None, false)
            .await
    }

    pub async fn upload_bytes_with_encoding(
        &self,
        key: &str,
        bytes: Bytes,
        content_type: &str,
        content_encoding: Option<&str>,
    ) -> Result<()> {
        self.upload_bytes_with_encoding_internal(key, bytes, content_type, content_encoding, true)
            .await
    }

    async fn upload_bytes_with_encoding_internal(
        &self,
        key: &str,
        bytes: Bytes,
        content_type: &str,
        content_encoding: Option<&str>,
        emit_logs: bool,
    ) -> Result<()> {
        debug_storage_marker("storage-upload-enter");
        if emit_logs {
            tracing::info!(
                storage_key = key,
                bytes = bytes.len(),
                content_type,
                content_encoding = content_encoding.unwrap_or(""),
                backend = if self.is_s3() { "s3" } else { "local" },
                "storage upload starting"
            );
        }

        match &self.backend {
            StorageBackend::Local { root } => {
                debug_storage_marker("storage-upload-local-enter");
                let path = path_for_key(root, key);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).await?;
                }

                fs::write(path, bytes).await?;
                let _ = (content_type, content_encoding);
                debug_storage_marker("storage-upload-local-after-write");
                if emit_logs {
                    tracing::info!(
                        storage_key = key,
                        backend = "local",
                        "storage upload finished"
                    );
                }
                debug_storage_marker("storage-upload-local-exit");
                Ok(())
            }
            StorageBackend::S3 { bucket, client, .. } => {
                debug_storage_marker("storage-upload-s3-enter");
                let mut request = client
                    .put_object()
                    .bucket(bucket)
                    .key(key)
                    .acl(ObjectCannedAcl::PublicRead)
                    .content_type(content_type)
                    .body(ByteStream::from(bytes.to_vec()));
                debug_storage_marker("storage-upload-s3-after-request-build");
                if let Some(content_encoding) = content_encoding {
                    request = request.content_encoding(content_encoding);
                }
                debug_storage_marker("storage-upload-s3-before-send");
                request.send().await?;
                debug_storage_marker("storage-upload-s3-after-send");
                if emit_logs {
                    tracing::info!(
                        storage_key = key,
                        bucket,
                        backend = "s3",
                        "storage upload finished"
                    );
                }
                debug_storage_marker("storage-upload-s3-exit");
                Ok(())
            }
        }
    }

    pub async fn get_bytes(&self, key: &str) -> Result<(Bytes, Option<String>, Option<String>)> {
        match &self.backend {
            StorageBackend::Local { root } => {
                let path = path_for_key(root, key);
                let bytes = fs::read(path).await?;
                Ok((Bytes::from(bytes), None, None))
            }
            StorageBackend::S3 { bucket, client, .. } => {
                let response = client.get_object().bucket(bucket).key(key).send().await?;
                let content_type = response.content_type().map(ToOwned::to_owned);
                let content_encoding = response.content_encoding().map(ToOwned::to_owned);
                let bytes = response.body.collect().await?.into_bytes();
                Ok((bytes, content_type, content_encoding))
            }
        }
    }

    pub async fn delete_key(&self, key: &str) -> Result<()> {
        match &self.backend {
            StorageBackend::Local { root } => {
                let path = path_for_key(root, key);
                match fs::remove_file(path).await {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error.into()),
                }
            }
            StorageBackend::S3 { bucket, client, .. } => {
                client
                    .delete_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await?;
                Ok(())
            }
        }
    }

    pub fn is_s3(&self) -> bool {
        matches!(self.backend, StorageBackend::S3 { .. })
    }

    pub fn public_url(&self, key: &str) -> Option<String> {
        match &self.backend {
            StorageBackend::Local { .. } => None,
            StorageBackend::S3 {
                public_base_url, ..
            } => Some(format!(
                "{}/{}",
                public_base_url.trim_end_matches('/'),
                key.trim_start_matches('/'),
            )),
        }
    }

    pub fn local_path_for_key(&self, key: &str) -> Option<PathBuf> {
        match &self.backend {
            StorageBackend::Local { root } => Some(path_for_key(root, key)),
            StorageBackend::S3 { .. } => None,
        }
    }
}

fn build_s3_client(config: &S3Config) -> Client {
    let mut s3_config = aws_sdk_s3::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(config.region.clone()))
        .stalled_stream_protection(StalledStreamProtectionConfig::disabled())
        .credentials_provider(Credentials::new(
            &config.access_key_id,
            &config.secret_access_key,
            None,
            None,
            "static-config",
        ));

    if let Some(endpoint) = &config.endpoint {
        s3_config = s3_config.endpoint_url(endpoint);
    }

    if config.force_path_style {
        s3_config = s3_config.force_path_style(true);
    }

    Client::from_conf(s3_config.build())
}

fn build_s3_public_base_url(config: &S3Config) -> String {
    if let Some(endpoint) = &config.endpoint {
        let endpoint = endpoint.trim_end_matches('/');

        if config.force_path_style {
            return format!("{}/{}", endpoint, config.bucket);
        }

        if let Some((scheme, rest)) = endpoint.split_once("://") {
            if rest.starts_with(&format!("{}.", config.bucket)) {
                endpoint.to_owned()
            } else {
                format!("{scheme}://{}.{}", config.bucket, rest)
            }
        } else if endpoint.starts_with(&format!("{}.", config.bucket)) {
            format!("https://{endpoint}")
        } else {
            format!("https://{}.{}", config.bucket, endpoint)
        }
    } else {
        format!(
            "https://{}.s3.{}.amazonaws.com",
            config.bucket, config.region
        )
    }
}

fn path_for_key(root: &Path, key: &str) -> PathBuf {
    key.split('/')
        .filter(|segment| !segment.is_empty())
        .fold(root.to_path_buf(), |path, segment| path.join(segment))
}

fn debug_storage_marker(message: &str) {
    if std::env::var("PROCESSOR_DEBUG_MARKERS")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
    {
        eprintln!("[processor-debug] {message}");
    }
}
