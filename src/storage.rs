use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_config::meta::region::RegionProviderChain;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Region};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use tokio::io::AsyncWriteExt;

use crate::config::Config;

#[derive(Clone)]
pub struct AssetStorage {
    backend: Arc<StorageBackend>,
}

enum StorageBackend {
    Local { root: PathBuf },
    S3 { client: S3Client, bucket: String },
}

impl AssetStorage {
    pub async fn from_config(config: &Config) -> Result<Self> {
        if let Some(bucket) = config.asset_bucket.clone() {
            let region_provider =
                RegionProviderChain::first_try(Some(Region::new(config.aws_region.clone())));
            let mut loader =
                aws_config::defaults(BehaviorVersion::latest()).region(region_provider);
            if let (Some(access_key), Some(secret_key)) = (
                config.aws_access_key_id.as_ref(),
                config.aws_secret_access_key.as_ref(),
            ) {
                loader = loader.credentials_provider(Credentials::new(
                    access_key,
                    secret_key,
                    None,
                    None,
                    "kitchensink",
                ));
            }
            let shared = loader.load().await;
            let mut builder = S3ConfigBuilder::from(&shared);
            if let Some(endpoint_url) = config.aws_endpoint_url_s3.as_ref() {
                builder = builder.endpoint_url(endpoint_url);
            }
            let client = S3Client::from_conf(builder.build());
            Ok(Self {
                backend: Arc::new(StorageBackend::S3 { client, bucket }),
            })
        } else {
            tokio::fs::create_dir_all(&config.local_asset_dir)
                .await
                .with_context(|| format!("create {}", config.local_asset_dir.display()))?;
            Ok(Self {
                backend: Arc::new(StorageBackend::Local {
                    root: config.local_asset_dir.clone(),
                }),
            })
        }
    }

    pub async fn put_bytes(&self, key: &str, content_type: &str, bytes: Bytes) -> Result<()> {
        match self.backend.as_ref() {
            StorageBackend::Local { root } => {
                let path = root.join(key);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .with_context(|| format!("create {}", parent.display()))?;
                }
                let mut file = tokio::fs::File::create(&path)
                    .await
                    .with_context(|| format!("create {}", path.display()))?;
                file.write_all(&bytes)
                    .await
                    .with_context(|| format!("write {}", path.display()))?;
                Ok(())
            }
            StorageBackend::S3 { client, bucket } => {
                client
                    .put_object()
                    .bucket(bucket)
                    .key(key)
                    .content_type(content_type)
                    .body(ByteStream::from(bytes))
                    .send()
                    .await
                    .with_context(|| format!("upload s3://{bucket}/{key}"))?;
                Ok(())
            }
        }
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Bytes> {
        match self.backend.as_ref() {
            StorageBackend::Local { root } => {
                let path = root.join(key);
                let bytes = tokio::fs::read(&path)
                    .await
                    .with_context(|| format!("read {}", path.display()))?;
                Ok(Bytes::from(bytes))
            }
            StorageBackend::S3 { client, bucket } => {
                let response = client
                    .get_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await
                    .with_context(|| format!("download s3://{bucket}/{key}"))?;
                let body = response.body.collect().await.context("collect s3 body")?;
                Ok(body.into_bytes())
            }
        }
    }
}
