use std::fmt;
use aws_sdk_s3::Error as AwsS3Error;
use aws_sdk_s3::error::{SdkError, ProvideErrorMetadata};
use thiserror::Error;


/// Результат операций с S3
pub type Result<T> = std::result::Result<T, S3Error>;

/// Ошибки, связанные с S3 операциями
#[derive(Error, Debug)]
pub enum S3Error {
    #[error("S3 service error: {0}")]
    AwsError(String),

    #[error("Failed to upload file: {0}")]
    UploadError(String),

    #[error("Failed to download file: {0}")]
    DownloadError(String),

    #[error("Failed to create multipart upload: {0}")]
    MultipartCreateError(String),

    #[error("Failed to upload part: {0}")]
    PartUploadError(String),

    #[error("Failed to complete multipart upload: {0}")]
    MultipartCompleteError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Object not found: bucket={bucket}, key={key}")]
    ObjectNotFound { bucket: String, key: String },

    #[error("Other error: {0}")]
    Other(String),
}

// Только обобщенная реализация
impl<E> From<SdkError<E>> for S3Error
where
    E: std::error::Error + ProvideErrorMetadata, // Добавляем трейт ProvideErrorMetadata
{
    fn from(err: SdkError<E>) -> Self {
        match &err {
            SdkError::ServiceError(service_err) => {
                // Теперь метод code() будет доступен
                let code = match service_err.err().code() {
                    Some(code) => code,
                    None => "UnknownError",
                };

                if code == "NoSuchKey" || code == "NoSuchBucket" {
                    return S3Error::ObjectNotFound {
                        bucket: "unknown".to_string(),
                        key: "unknown".to_string()
                    };
                }
                S3Error::AwsError(format!("AWS service error: {code}"))
            },
            _ => S3Error::AwsError(format!("AWS SDK error: {}", err)),
        }
    }
}

impl From<anyhow::Error> for S3Error {
    fn from(err: anyhow::Error) -> Self {
        S3Error::Other(err.to_string())
    }
}
