pub mod s3_old;

pub mod s3;


use s3::{S3Manager};
use anyhow::Result;
use my_core::config::CONFIG;

#[derive(Debug, Clone)]
pub struct AppState {
    pub s3: S3Manager,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        // Create s3_old manager
        let region = CONFIG.s3_region_name.clone();
        let endpoint = CONFIG.s3_endpoint.to_str().expect("Invalid S3 endpoint").to_string();
        let access_key = CONFIG.s3_svaha_writer_login.clone();
        let secret_key = CONFIG.s3_svaha_writer_password.clone();

        let credentials = aws_sdk_s3::config::Credentials::new(
            access_key,
            secret_key,
            None,
            None,
            "env",
        );

        let s3 = S3Manager::new(region, Some(endpoint), credentials).await?;

        Ok(Self { s3 })
    }
}