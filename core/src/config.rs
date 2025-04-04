use std::env;
use axum::http::HeaderValue;
use std::net::Ipv4Addr;

use clap::builder::Str;
use clap::builder::styling::Reset;
use tracing;

use clap::Parser;
use dotenv::dotenv;

use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

pub static CONFIG: Lazy<Config> = Lazy::new(|| {Config::new().unwrap()});

#[derive(Parser)]
pub struct Config {

    #[arg(long, env)]
    pub host: Ipv4Addr,
    #[arg(long, env)]
    pub port: u16,
    #[arg(long, env)]
    pub api_v1_str: String,

    #[arg(long, env)]
    pub redis_host: HeaderValue,
    #[arg(long, env)]
    pub redis_port: u16,
    #[arg(long, env)]
    pub redis_login: String,
    #[arg(long, env)]
    pub redis_password: String,


    #[arg(long, env)]
    pub s3_endpoint: HeaderValue,
    #[arg(long, env)]
    pub s3_svaha_writer_login: String,
    #[arg(long, env)]
    pub s3_svaha_writer_password: String,
    #[arg(long, env)]
    pub s3_bucket_name: String,
    #[arg(long, env)]
    pub s3_region_name: String,
    #[arg(long, env, default_value = "")]
    pub upload_public_domain: String,

    #[arg(long, env, default_value = "./")]
    pub base_upload_dir: String,
}


impl Config {
    pub fn new() -> Result<Config, String> {
        let bin_directory = bin_path()?;
        let bin_env_path = format!("{}/", bin_directory.display());

        let locations = vec![
            "./".to_string(),
            "../".to_string(),
            "../../".to_string(),
            bin_env_path,
        ];

        for location in locations {
            let env_path = Path::new(&location).join(".env");
            if env_path.exists() {
                match dotenv::from_path(&env_path) {
                    Ok(_) => {
                        tracing::debug!("Using config from: {}", env_path.display());
                        // return Ok(Config::parse());
                        return Config::parse_and_set_defaults();
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load .env from {}: {}", env_path.display(), e);
                    }
                }
            }
        }

        // If no .env file found, try to load from environment
        match dotenv() {
            Ok(_) => {
                tracing::debug!("Using config from environment variables");
                Config::parse_and_set_defaults()
                // Ok(Config::parse())
            }
            Err(e) => {
                Err(format!("Failed to load any .env file or environment variables: {}", e))
            }
        }
    }
    fn parse_and_set_defaults() -> Result<Config, String> {
        let mut config: Config = Config::parse();

        if config.upload_public_domain.is_empty() {
            config.upload_public_domain = format!("http://127.0.0.1:{}", config.port);
            // config.upload_public_domain = format!("http://{}:{}", "192.168.1.45", 8023);
        }

        Ok(config)
    }
}

fn bin_path() -> Result<PathBuf, String> {
    env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))
}