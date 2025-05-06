use std::sync::Arc;
use aws_sdk_s3::{Client};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use bytes::Bytes;
use tokio::sync::Mutex;
use super::errors::{Result, S3Error};

/// Опции для мультичастной загрузки
#[derive(Debug, Clone)]
pub struct MultipartUploadOptions {
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
    pub chunk_size: usize,
}

impl Default for MultipartUploadOptions {
    fn default() -> Self {
        Self {
            content_type: None,
            content_disposition: None,
            chunk_size: 5 * 1024 * 1024, // 5MB по умолчанию
        }
    }
}

/// Контекст для мультичастной загрузки файла
#[derive(Debug)]
pub struct MultipartUploadContext {
    client: Client,
    bucket: String,
    key: String,
    upload_id: String,
    parts: Arc<Mutex<Vec<CompletedPart>>>,
}

impl MultipartUploadContext {
    /// Создает новый контекст мультичастной загрузки
    pub(crate) async fn new(
        client: Client,
        bucket: &str,
        key: &str,
        options: Option<MultipartUploadOptions>
    ) -> Result<Self> {
        let options = options.unwrap_or_default();

        let mut create_req = client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key);

        if let Some(content_type) = options.content_type {
            create_req = create_req.content_type(content_type);
        }

        if let Some(disposition) = options.content_disposition {
            create_req = create_req.content_disposition(disposition);
        }

        let output = create_req
            .send()
            .await
            .map_err(|err| S3Error::MultipartCreateError(err.to_string()))?;

        let upload_id = output
            .upload_id()
            .ok_or_else(|| S3Error::MultipartCreateError("No upload ID returned".to_string()))?
            .to_string();

        Ok(Self {
            client,
            bucket: bucket.to_string(),
            key: key.to_string(),
            upload_id,
            parts: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Загружает часть файла
    pub async fn upload_part(&self, part_number: i32, body: Bytes) -> Result<()> {
        let result = self.client
            .upload_part()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .part_number(part_number)
            .body(body.into())
            .send()
            .await
            .map_err(|err| S3Error::PartUploadError(format!("Part {}: {}", part_number, err)))?;

        let etag = result
            .e_tag()
            .ok_or_else(|| S3Error::PartUploadError(format!("No ETag returned for part {}", part_number)))?;

        let part = CompletedPart::builder()
            .e_tag(etag)
            .part_number(part_number)
            .build();

        let mut parts = self.parts.lock().await;
        parts.push(part);

        Ok(())
    }

    /// Завершает мультичастную загрузку
    pub async fn complete(&self) -> Result<()> {
        let parts = {
            let parts = self.parts.lock().await;
            parts.clone()
        };

        // Сортируем части по номеру для корректной сборки файла
        let mut sorted_parts = parts;
        sorted_parts.sort_by_key(|part| part.part_number());

        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(sorted_parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .map_err(|err| S3Error::MultipartCompleteError(err.to_string()))?;

        Ok(())
    }

    /// Отменяет мультичастную загрузку, если что-то пошло не так
    pub async fn abort(&self) -> Result<()> {
        self.client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .send()
            .await
            .map_err(|err| S3Error::Other(format!("Failed to abort multipart upload: {}", err)))?;

        Ok(())
    }
}


// use std::sync::Arc;
// use aws_sdk_s3::{Client, operation::create_multipart_upload::CreateMultipartUploadOutput};
// use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
// use bytes::Bytes;
// use super::errors::{Result, S3Error};
// use tokio::sync::Mutex;
//
// /// Опции для мультичастной загрузки
// #[derive(Debug, Clone)]
// pub struct MultipartUploadOptions {
//     pub content_type: Option<String>,
//     pub content_disposition: Option<String>,
//     pub chunk_size: usize,
// }
//
// impl Default for MultipartUploadOptions {
//     fn default() -> Self {
//         Self {
//             content_type: None,
//             content_disposition: None,
//             chunk_size: 5 * 1024 * 1024, // 5MB по умолчанию
//         }
//     }
// }
//
// /// Контекст для мультичастной загрузки файла
// #[derive(Debug)]
// pub struct MultipartUploadContext {
//     client: Client,
//     bucket: String,
//     key: String,
//     upload_id: String,
//     parts: Vec<CompletedPart>,
// }
//
// impl MultipartUploadContext {
//     /// Создает новый контекст мультичастной загрузки
//     pub(crate) async fn new(
//         client: Client,
//         bucket: &str,
//         key: &str,
//         options: Option<MultipartUploadOptions>
//     ) -> Result<Self> {
//         let options = options.unwrap_or_default();
//
//         let mut create_req = client
//             .create_multipart_upload()
//             .bucket(bucket)
//             .key(key);
//
//         if let Some(content_type) = options.content_type {
//             create_req = create_req.content_type(content_type);
//         }
//
//         if let Some(disposition) = options.content_disposition {
//             create_req = create_req.content_disposition(disposition);
//         }
//
//         let output = create_req
//             .send()
//             .await
//             .map_err(|err| S3Error::MultipartCreateError(err.to_string()))?;
//
//         let upload_id = output
//             .upload_id()
//             .ok_or_else(|| S3Error::MultipartCreateError("No upload ID returned".to_string()))?
//             .to_string();
//
//         Ok(Self {
//             client,
//             bucket: bucket.to_string(),
//             key: key.to_string(),
//             upload_id,
//             parts: Vec::new(),
//         })
//     }
//
//     /// Загружает часть файла - теперь принимает &self вместо &mut self
//     pub async fn upload_part(&self, part_number: i32, body: Bytes) -> Result<()> {
//         let result = self.client
//             .upload_part()
//             .bucket(&self.bucket)
//             .key(&self.key)
//             .upload_id(&self.upload_id)
//             .part_number(part_number)
//             .body(body.into())
//             .send()
//             .await
//             .map_err(|err| S3Error::PartUploadError(format!("Part {}: {}", part_number, err)))?;
//
//         let etag = result
//             .e_tag()
//             .ok_or_else(|| S3Error::PartUploadError(format!("No ETag returned for part {}", part_number)))?;
//
//         let part = CompletedPart::builder()
//             .e_tag(etag)
//             .part_number(part_number)
//             .build();
//
//         // Защищаем вектор мьютексом только на время обновления
//         let mut parts = self.parts.clone();
//         parts.push(part);
//
//         Ok(())
//     }
//
//     /// Завершает мультичастную загрузку
//     pub async fn complete(&self) -> Result<()> {
//         // Получаем все части из защищенного вектора
//         let parts = self.parts.clone();
//
//         // Сортируем части по номеру
//         let mut sorted_parts = parts;
//         sorted_parts.sort_by_key(|part| part.part_number());
//
//         let completed_upload = CompletedMultipartUpload::builder()
//             .set_parts(Some(sorted_parts))
//             .build();
//
//         self.client
//             .complete_multipart_upload()
//             .bucket(&self.bucket)
//             .key(&self.key)
//             .upload_id(&self.upload_id)
//             .multipart_upload(completed_upload)
//             .send()
//             .await
//             .map_err(|err| S3Error::MultipartCompleteError(err.to_string()))?;
//
//         Ok(())
//     }
//
//     /// Отменяет мультичастную загрузку, если что-то пошло не так
//     pub async fn abort(&self) -> Result<()> {
//         self.client
//             .abort_multipart_upload()
//             .bucket(&self.bucket)
//             .key(&self.key)
//             .upload_id(&self.upload_id)
//             .send()
//             .await
//             .map_err(|err| S3Error::Other(format!("Failed to abort multipart upload: {}", err)))?;
//
//         Ok(())
//     }
// }


// use std::rc::Rc;
// use std::cell::RefCell;
// use aws_sdk_s3::{Client};
// use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
// use bytes::Bytes;
// use super::errors::{Result, S3Error};

/// Опции для мультичастной загрузки
// #[derive(Debug, Clone)]
// pub struct MultipartUploadOptions {
//     pub content_type: Option<String>,
//     pub content_disposition: Option<String>,
//     pub chunk_size: usize,
// }
// 
// impl Default for MultipartUploadOptions {
//     fn default() -> Self {
//         Self {
//             content_type: None,
//             content_disposition: None,
//             chunk_size: 5 * 1024 * 1024, // 5MB по умолчанию
//         }
//     }
// }

/// Контекст для мультичастной загрузки файла
// #[derive(Debug)]
// pub struct MultipartUploadContext {
//     client: Client,
//     bucket: String,
//     key: String,
//     upload_id: String,
//     parts: Rc<RefCell<Vec<CompletedPart>>>,
// }
// 
// impl MultipartUploadContext {
//     /// Создает новый контекст мультичастной загрузки
//     pub(crate) async fn new(
//         client: Client,
//         bucket: &str,
//         key: &str,
//         options: Option<MultipartUploadOptions>
//     ) -> Result<Self> {
//         let options = options.unwrap_or_default();
// 
//         let mut create_req = client
//             .create_multipart_upload()
//             .bucket(bucket)
//             .key(key);
// 
//         if let Some(content_type) = options.content_type {
//             create_req = create_req.content_type(content_type);
//         }
// 
//         if let Some(disposition) = options.content_disposition {
//             create_req = create_req.content_disposition(disposition);
//         }
// 
//         let output = create_req
//             .send()
//             .await
//             .map_err(|err| S3Error::MultipartCreateError(err.to_string()))?;
// 
//         let upload_id = output
//             .upload_id()
//             .ok_or_else(|| S3Error::MultipartCreateError("No upload ID returned".to_string()))?
//             .to_string();
// 
//         Ok(Self {
//             client,
//             bucket: bucket.to_string(),
//             key: key.to_string(),
//             upload_id,
//             parts: Rc::new(RefCell::new(Vec::new())),
//         })
//     }
// 
//     /// Загружает часть файла
//     pub async fn upload_part(&self, part_number: i32, body: Bytes) -> Result<()> {
//         let result = self.client
//             .upload_part()
//             .bucket(&self.bucket)
//             .key(&self.key)
//             .upload_id(&self.upload_id)
//             .part_number(part_number)
//             .body(body.into())
//             .send()
//             .await
//             .map_err(|err| S3Error::PartUploadError(format!("Part {}: {}", part_number, err)))?;
// 
//         let etag = result
//             .e_tag()
//             .ok_or_else(|| S3Error::PartUploadError(format!("No ETag returned for part {}", part_number)))?;
// 
//         let part = CompletedPart::builder()
//             .e_tag(etag)
//             .part_number(part_number)
//             .build();
// 
//         // Используем RefCell вместо Mutex
//         self.parts.borrow_mut().push(part);
// 
//         Ok(())
//     }
// 
//     /// Завершает мультичастную загрузку
//     pub async fn complete(&self) -> Result<()> {
//         // Получаем копию всех частей
//         let parts = self.parts.borrow().clone();
// 
//         // Сортируем части по номеру для корректной сборки файла
//         let mut sorted_parts = parts;
//         sorted_parts.sort_by_key(|part| part.part_number());
// 
//         let completed_upload = CompletedMultipartUpload::builder()
//             .set_parts(Some(sorted_parts))
//             .build();
// 
//         self.client
//             .complete_multipart_upload()
//             .bucket(&self.bucket)
//             .key(&self.key)
//             .upload_id(&self.upload_id)
//             .multipart_upload(completed_upload)
//             .send()
//             .await
//             .map_err(|err| S3Error::MultipartCompleteError(err.to_string()))?;
// 
//         Ok(())
//     }
// 
//     /// Отменяет мультичастную загрузку, если что-то пошло не так
//     pub async fn abort(&self) -> Result<()> {
//         self.client
//             .abort_multipart_upload()
//             .bucket(&self.bucket)
//             .key(&self.key)
//             .upload_id(&self.upload_id)
//             .send()
//             .await
//             .map_err(|err| S3Error::Other(format!("Failed to abort multipart upload: {}", err)))?;
// 
//         Ok(())
//     }
// }


impl Clone for MultipartUploadContext {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            bucket: self.bucket.clone(),
            key: self.key.clone(),
            upload_id: self.upload_id.clone(),
            parts: self.parts.clone(),
        }
    }
}