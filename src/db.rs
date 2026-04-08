use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use surrealdb::{
    Surreal,
    engine::any::{self, Any},
    opt::auth::Root,
};

use crate::auth::hash_password;
use crate::config::Config;
use crate::models::{CountRow, StoredModelSettings};

pub type RawSurrealDb = Surreal<Any>;

#[derive(Clone)]
pub struct SurrealDb {
    client: RawSurrealDb,
}

impl SurrealDb {
    pub fn client(&self) -> RawSurrealDb {
        self.client.clone()
    }
}

pub async fn connect(config: &Config) -> Result<SurrealDb> {
    let client = any::connect(&config.surreal_url)
        .await
        .with_context(|| format!("connect surrealdb {}", config.surreal_url))?;

    if let Some(token) = config.surreal_token.as_deref() {
        client
            .authenticate(token)
            .await
            .context("authenticate surrealdb token")?;
    } else if let (Some(username), Some(password)) = (
        config.surreal_username.as_deref(),
        config.surreal_password.as_deref(),
    ) {
        client
            .signin(Root {
                username: username.to_string(),
                password: password.to_string(),
            })
            .await
            .context("signin surrealdb root credentials")?;
    } else {
        return Err(anyhow!(
            "SurrealDB auth is required: set SURREALDB_TOKEN or SURREALDB_USERNAME/SURREALDB_PASSWORD"
        ));
    }

    client
        .use_ns(config.surreal_namespace.clone())
        .use_db(config.surreal_database.clone())
        .await
        .context("select surrealdb namespace/database")?;

    Ok(SurrealDb { client })
}

pub async fn apply_migrations(db: &SurrealDb) -> Result<()> {
    let migration_dir = PathBuf::from("db/migrations");
    let mut entries = tokio::fs::read_dir(&migration_dir)
        .await
        .with_context(|| format!("read {}", migration_dir.display()))?;
    let mut files = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_file() {
            files.push(entry.path());
        }
    }

    files.sort();
    for path in files {
        let sql = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read {}", path.display()))?;
        db.client()
            .query(sql)
            .await
            .with_context(|| format!("apply migration {}", path.display()))?;
    }

    Ok(())
}

pub async fn bootstrap_defaults(db: &SurrealDb, config: &Config) -> Result<()> {
    ensure_model_settings(db, config).await?;
    ensure_admin_user(db, config).await?;
    Ok(())
}

async fn ensure_model_settings(db: &SurrealDb, config: &Config) -> Result<()> {
    let mut response = db
        .client()
        .query("SELECT * FROM type::record('app_setting', 'model');")
        .await
        .context("load model settings")?;
    let existing: Option<StoredModelSettings> = response.take(0).context("parse model settings")?;
    if existing.is_some() {
        return Ok(());
    }

    let now = Utc::now();
    db.client()
        .query(
            "CREATE type::record('app_setting', 'model') CONTENT {
                base_url: $base_url,
                api_key: $api_key,
                model: $model,
                updated_at: $updated_at
            };",
        )
        .bind(("base_url", config.default_model_base_url.clone()))
        .bind(("api_key", config.default_model_api_key.clone()))
        .bind(("model", config.default_model_name.clone()))
        .bind(("updated_at", now))
        .await
        .context("create default model settings")?;

    Ok(())
}

async fn ensure_admin_user(db: &SurrealDb, config: &Config) -> Result<()> {
    let mut response = db
        .client()
        .query("SELECT count() AS count FROM auth_user GROUP ALL;")
        .await
        .context("count auth users")?;
    let counts: Vec<CountRow> = response.take(0).context("parse auth user count")?;
    if counts.first().map(|row| row.count).unwrap_or_default() > 0 {
        return Ok(());
    }

    let Some(password) = config.admin_password.as_deref() else {
        tracing::warn!(
            "no admin password configured; login will remain unavailable until APP_PASSWORD is set"
        );
        return Ok(());
    };

    let password_hash = hash_password(password).context("hash bootstrap admin password")?;
    let now = Utc::now();
    db.client()
        .query(
            "CREATE type::record('auth_user', $id) CONTENT {
                username: $username,
                password_hash: $password_hash,
                created_at: $created_at,
                updated_at: $updated_at
            };",
        )
        .bind(("id", config.admin_username.clone()))
        .bind(("username", config.admin_username.clone()))
        .bind(("password_hash", password_hash))
        .bind(("created_at", now))
        .bind(("updated_at", now))
        .await
        .context("create bootstrap admin user")?;

    tracing::info!(username = %config.admin_username, "bootstrapped admin user");
    Ok(())
}
