use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub public_base_url: String,
    pub frontend_dir: PathBuf,
    pub app_name: String,
    pub session_cookie_name: String,
    pub admin_username: String,
    pub admin_password: Option<String>,
    pub surreal_url: String,
    pub surreal_namespace: String,
    pub surreal_database: String,
    pub surreal_username: Option<String>,
    pub surreal_password: Option<String>,
    pub surreal_token: Option<String>,
    pub default_model_base_url: String,
    pub default_model_name: String,
    pub default_model_api_key: Option<String>,
    pub default_model_max_context_tokens: usize,
    pub tavily_api_key: Option<String>,
    pub asset_bucket: Option<String>,
    pub aws_endpoint_url_s3: Option<String>,
    pub aws_region: String,
    pub aws_access_key_id: Option<String>,
    pub aws_secret_access_key: Option<String>,
    pub local_asset_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_addr = env_string("BIND_ADDR", "0.0.0.0:8080")
            .parse::<SocketAddr>()
            .context("parse BIND_ADDR")?;
        let public_base_url = env_string("PUBLIC_BASE_URL", "http://127.0.0.1:8080");
        let frontend_dir = PathBuf::from(env_string("FRONTEND_DIR", "web/dist"));
        let app_name = env_string("APP_NAME", "Kitchensink");
        let session_cookie_name = env_string("SESSION_COOKIE_NAME", "kitchensink_session");
        let admin_username = env_string("ADMIN_USERNAME", "operator");
        let admin_password = opt_env("APP_PASSWORD")
            .or_else(|| opt_env("ADMIN_PASSWORD"))
            .or_else(dev_password_fallback);

        let surreal_url =
            opt_env("SURREALDB_URL").ok_or_else(|| anyhow!("SURREALDB_URL is required"))?;
        let surreal_namespace = env_string("SURREALDB_NAMESPACE", "main");
        let surreal_database = env_string("SURREALDB_DATABASE", "main");
        let surreal_username = opt_env("SURREALDB_USERNAME");
        let surreal_password = opt_env("SURREALDB_PASSWORD");
        let surreal_token = opt_env("SURREALDB_TOKEN");

        let default_model_base_url = env_string(
            "OPENAI_BASE_URL",
            env_string("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
        );
        let default_model_name = env_string("OPENAI_MODEL", "openai/gpt-5.4");
        let default_model_api_key =
            opt_env("OPENAI_API_KEY").or_else(|| opt_env("OPENROUTER_API_KEY"));
        let default_model_max_context_tokens = env_string("MODEL_MAX_CONTEXT_TOKENS", "120000")
            .parse::<usize>()
            .context("parse MODEL_MAX_CONTEXT_TOKENS")?;

        let tavily_api_key = opt_env("TAVILY_API_KEY");
        let asset_bucket = opt_env("BUCKET_NAME");
        let aws_endpoint_url_s3 = opt_env("AWS_ENDPOINT_URL_S3");
        let aws_region = env_string("AWS_REGION", "auto");
        let aws_access_key_id = opt_env("AWS_ACCESS_KEY_ID");
        let aws_secret_access_key = opt_env("AWS_SECRET_ACCESS_KEY");
        let local_asset_dir = PathBuf::from(env_string("LOCAL_ASSET_DIR", ".data/assets"));

        Ok(Self {
            bind_addr,
            public_base_url,
            frontend_dir,
            app_name,
            session_cookie_name,
            admin_username,
            admin_password,
            surreal_url,
            surreal_namespace,
            surreal_database,
            surreal_username,
            surreal_password,
            surreal_token,
            default_model_base_url,
            default_model_name,
            default_model_api_key,
            default_model_max_context_tokens,
            tavily_api_key,
            asset_bucket,
            aws_endpoint_url_s3,
            aws_region,
            aws_access_key_id,
            aws_secret_access_key,
            local_asset_dir,
        })
    }

    pub fn secure_cookies(&self) -> bool {
        self.public_base_url.starts_with("https://")
    }

    pub fn allowed_origins(&self) -> Vec<String> {
        let mut origins = vec![
            "http://127.0.0.1:8080".to_string(),
            "http://localhost:8080".to_string(),
            "http://127.0.0.1:5173".to_string(),
            "http://localhost:5173".to_string(),
        ];

        if let Ok(url) = Url::parse(&self.public_base_url) {
            if let Some(origin) = url.origin().ascii_serialization().strip_suffix('/') {
                origins.push(origin.to_string());
            } else {
                origins.push(url.origin().ascii_serialization());
            }
        }

        origins.sort();
        origins.dedup();
        origins
    }
}

fn opt_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_string(name: &str, default: impl Into<String>) -> String {
    opt_env(name).unwrap_or_else(|| default.into())
}

fn dev_password_fallback() -> Option<String> {
    let app_env = opt_env("APP_ENV").unwrap_or_else(|| "development".to_string());
    if app_env.eq_ignore_ascii_case("production") {
        None
    } else {
        Some("dev-password".to_string())
    }
}
