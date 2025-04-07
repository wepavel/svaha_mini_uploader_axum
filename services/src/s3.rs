use std::collections::HashSet;
use std::path::Path;
use aws_sdk_s3::{Client, Config, config::{Credentials, Region, BehaviorVersion}};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, Delete, ObjectIdentifier};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::create_multipart_upload::CreateMultipartUploadOutput;
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::operation::put_object::PutObjectOutput;
use aws_sdk_s3::operation::create_bucket::CreateBucketOutput;
use async_trait::async_trait;
use anyhow::{Result, anyhow};
use bytes::{Bytes, BytesMut};
use ulid::Ulid;
use tracing;
use futures::{Stream, StreamExt};
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;
/// Manages interactions with Amazon S3 or compatible object storage services.
#[derive(Debug, Clone)]
pub struct S3Manager {
    client: Client,
}

impl S3Manager {
    /// Creates a new S3Manager instance.
    ///
    /// # Arguments
    ///
    /// * `region` - The AWS region as a string.
    /// * `endpoint` - An optional endpoint URL for the S3 service.
    /// * `credentials` - AWS credentials for authentication.
    ///
    /// # Returns
    ///
    /// A Result containing the new S3Manager instance or an error.
    ///
    /// # Example
    ///
    /// ```
    /// use aws_sdk_s3::config::Credentials;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let region = "us-west-2".to_string();
    ///     let endpoint = Some("https://s3.amazonaws.com".to_string());
    ///     let credentials = Credentials::new(
    ///         "your_access_key",
    ///         "your_secret_key",
    ///         None,
    ///         None,
    ///         "example-provider"
    ///     );
    ///
    ///     let s3_manager = S3Manager::new(region, endpoint, credentials).await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn new(region: String, endpoint: Option<String>, credentials: Credentials) -> Result<Self> {
        let region = Region::new(region);
        let mut config_builder = Config::builder()
            .region(region)
            .credentials_provider(credentials)
            .behavior_version(BehaviorVersion::latest());

        if let Some(endpoint_url) = endpoint {
            config_builder = config_builder.endpoint_url(endpoint_url);
        }

        let config = config_builder.build();
        let client = Client::from_conf(config);

        Ok(Self { client })
    }

    /// Copies an object from one bucket to another.
    ///
    /// # Arguments
    ///
    /// * `source_bucket` - The name of the source bucket.
    /// * `destination_bucket` - The name of the destination bucket.
    /// * `source_object` - The key of the source object.
    /// * `destination_object` - The key for the destination object.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn copy_object(
        &self,
        source_bucket: &str,
        destination_bucket: &str,
        source_object: &str,
        destination_object: &str,
    ) -> Result<()> {
        let source_key = format!("{source_bucket}/{source_object}");
        let response = self.client
            .copy_object()
            .copy_source(&source_key)
            .bucket(destination_bucket)
            .key(destination_object)
            .send()
            .await?;

        let etag = response
            .copy_object_result
            .as_ref()
            .and_then(|result| result.e_tag())
            .unwrap_or("missing");

        tracing::info!(
            "Copied from {source_key} to {destination_bucket}/{destination_object} with etag {etag}"
        );
        Ok(())
    }

    /// Removes an object from a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket.
    /// * `key` - The key of the object to remove.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn remove_object(&self, bucket: &str, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }

    /// Downloads an object from a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket.
    /// * `key` - The key of the object to download.
    ///
    /// # Returns
    ///
    /// A Result containing the GetObjectOutput or an error.
    pub async fn download_object(&self, bucket_name: &str, key: &str) -> Result<GetObjectOutput> {
        Ok(self.client
            .get_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await?)
    }

    /// Uploads an object to a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket.
    /// * `file_name` - The path to the file to upload.
    /// * `key` - The key to assign to the uploaded object.
    ///
    /// # Returns
    ///
    /// A Result containing the PutObjectOutput or an error.
    pub async fn upload_object(&self, bucket_name: &str, file_name: &str, key: &str) -> Result<PutObjectOutput> {
        let body = aws_sdk_s3::primitives::ByteStream::from_path(Path::new(file_name)).await?;
        Ok(self.client
            .put_object()
            .bucket(bucket_name)
            .key(key)
            .body(body)
            .send()
            .await?)
    }
    /// Lists objects in a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn list_objects(&self, bucket: &str) -> Result<()> {
        let mut response = self.client
            .list_objects_v2()
            .bucket(bucket.to_owned())
            .max_keys(10)
            .into_paginator()
            .send();

        while let Some(result) = response.next().await {
            match result {
                Ok(output) => {
                    for object in output.contents() {
                        tracing::info!(" - {}", object.key().unwrap_or("Unknown"));
                    }
                }
                Err(err) => {
                    tracing::info!("{err:?}")
                }
            }
        }

        Ok(())
    }


    /// Checks if a file exists at the specified key in the bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket.
    /// * `key` - The object key to check.
    ///
    /// # Returns
    ///
    /// `Result<bool>` - `true` if the file exists, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the S3 request fails for reasons other than the file not existing.
    pub async fn is_file(&self, bucket_name: &str, key: &str) -> Result<bool> {
        match self.client
            .head_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                if let SdkError::ServiceError(service_error) = &err {
                    if service_error.err().is_not_found() {
                        return Ok(false);
                    }
                }
                Err(err.into())
            }
        }
    }

    pub async fn is_file_any_extension(
        &self,
        bucket_name: &str,
        key_prefix: &str,
        extensions: &[String]
    ) -> Result<bool> {
        // Сначала проверяем файл без расширения
        if self.is_file(bucket_name, key_prefix).await? {
            return Ok(true);
        }


        // Затем проверяем все предоставленные расширения в заданном порядке
        for ext in extensions {
            let key = format!("{}{}", key_prefix, ext);
            if self.is_file(bucket_name, &key).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Clears all objects from a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket to clear.
    ///
    /// # Returns
    ///
    /// A Result containing a vector of deleted object keys or an error.
    pub async fn clear_bucket(&self, bucket_name: &str) -> Result<Vec<String>> {
        let objects = self.client.list_objects_v2().bucket(bucket_name).send().await?;

        let objects_to_delete: Vec<String> = objects
            .contents()
            .iter()
            .filter_map(|obj| obj.key())
            .map(String::from)
            .collect();

        if objects_to_delete.is_empty() {
            return Ok(vec![]);
        }

        let return_keys = objects_to_delete.clone();

        self.delete_objects(bucket_name, objects_to_delete).await?;

        let objects = self.client.list_objects_v2().bucket(bucket_name).send().await?;

        match objects.key_count {
            Some(0) => Ok(return_keys),
            _ => Err(anyhow!("There were still objects left in the bucket.")),
        }
    }

    /// Deletes multiple objects from a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket.
    /// * `objects_to_delete` - A vector of object keys to delete.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn delete_objects(&self, bucket_name: &str, objects_to_delete: Vec<String>) -> Result<()> {
        let delete_object_ids: Vec<ObjectIdentifier> = objects_to_delete
            .into_iter()
            .map(|obj| {
                ObjectIdentifier::builder()
                    .key(obj)
                    .build()
                    .expect("Failed to build ObjectIdentifier")
            })
            .collect();

        self.client
            .delete_objects()
            .bucket(bucket_name)
            .delete(
                Delete::builder()
                    .set_objects(Some(delete_object_ids))
                    .build()
                    .map_err(|err| anyhow!("Failed to build delete_object input: {}", err))?,
            )
            .send()
            .await?;
        Ok(())
    }

    /// Creates a new bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket to create.
    /// * `region` - The AWS region for the bucket.
    ///
    /// # Returns
    ///
    /// A Result containing an Option<CreateBucketOutput> or an error.
    pub async fn create_bucket(&self, bucket_name: &str, region: &Region) -> Result<Option<CreateBucketOutput>> {
        let constraint = aws_sdk_s3::types::BucketLocationConstraint::from(region.as_ref());
        let cfg = aws_sdk_s3::types::CreateBucketConfiguration::builder()
            .location_constraint(constraint)
            .build();
        let create = self.client
            .create_bucket()
            .create_bucket_configuration(cfg)
            .bucket(bucket_name)
            .send()
            .await;

        create.map(Some).or_else(|err| {
            if err
                .as_service_error()
                .map(|se| se.is_bucket_already_exists() || se.is_bucket_already_owned_by_you())
                == Some(true)
            {
                Ok(None)
            } else {
                Err(err.into())
            }
        })
    }

    /// Deletes a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket_name` - The name of the bucket to delete.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn delete_bucket(&self, bucket_name: &str) -> Result<()> {
        let resp = self.client.delete_bucket().bucket(bucket_name).send().await;
        match resp {
            Ok(_) => Ok(()),
            Err(err) => {
                if err
                    .as_service_error()
                    .and_then(aws_sdk_s3::error::ProvideErrorMetadata::code)
                    == Some("NoSuchBucket")
                {
                    Ok(())
                } else {
                    Err(err.into())
                }
            }
        }
    }

    /// Создает новый контекст для мультипарт загрузки.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Имя бакета.
    /// * `key` - Ключ объекта.
    ///
    /// # Returns
    ///
    /// Result с MultipartUploadContext или ошибкой.
    /// # Example
    /// ```
    ///     #[tokio::main]
    ///     async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///         let s3_manager = S3Manager::new(/* ... */).await?;
    ///
    ///         // Начинаем мультипарт загрузку
    ///         let mut upload_context = s3_manager.start_multipart_upload("my-bucket", "my-key").await?;
    ///
    ///         // Загружаем части
    ///         upload_context.upload_part(1, Bytes::from("Part 1 data")).await?;
    ///
    ///         // Предположим, что здесь произошла ошибка или пользователь решил отменить загрузку
    ///         if some_condition {
    ///             // Отменяем загрузку
    ///             upload_context.abort().await?;
    ///             println!("Upload aborted");
    ///             return Ok(());
    ///         }
    ///
    ///         // Если всё в порядке, продолжаем загрузку
    ///         upload_context.upload_part(2, Bytes::from("Part 2 data")).await?;
    ///
    ///         // Завершаем мультипарт загрузку
    ///         let upload_result = upload_context.complete().await?;
    ///         println!("Upload completed. ETag: {}, Size: {}", upload_result.etag, upload_result.size);
    ///
    ///         Ok(())
    ///     }
    ///```
    pub async fn create_multipart_upload_context(&self, bucket: &str, key: &str) -> Result<MultipartUploadContext> {
        let create_output = self.create_multipart_upload(bucket, key).await?;
        let upload_id = create_output.upload_id().ok_or_else(|| anyhow!("Failed to get upload ID"))?;

        Ok(MultipartUploadContext {
            s3_manager: self.clone(),
            bucket: bucket.to_string(),
            key: key.to_string(),
            upload_id: upload_id.to_string(),
            parts: Vec::new(),
            total_size: 0,
        })
    }

    async fn create_multipart_upload(&self, bucket: &str, key: &str) -> Result<CreateMultipartUploadOutput> {
        Ok(self.client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key)
            .send()
            .await?)
    }

    async fn upload_part(&self, bucket: &str, key: &str, upload_id: &str, part_number: i32, body: Vec<u8>) -> Result<CompletedPart> {
        let response = self.client
            .upload_part()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(body.into())
            .send()
            .await?;

        Ok(CompletedPart::builder()
            .e_tag(response.e_tag.unwrap_or_default())
            .part_number(part_number)
            .build())
    }

    async fn complete_multipart_upload(&self, bucket: &str, key: &str, upload_id: &str, parts: Vec<CompletedPart>) -> Result<()> {
        let completed_multipart_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed_multipart_upload)
            .send()
            .await?;

        Ok(())
    }

    async fn abort_multipart_upload(&self, bucket: &str, key: &str, upload_id: &str) -> Result<()> {
        self.client
            .abort_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await?;

        Ok(())
    }
}


/// Represents the context for a multipart upload operation.
/// Example of how to use MultipartUploadContext
///
/// ```
/// use bytes::Bytes;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Assume s3_manager is already created
///     let s3_manager = S3Manager::new(/* ... */).await?;
///
///     // Start a multipart upload
///     let create_multipart_output = s3_manager.create_multipart_upload("my-bucket", "my-key").await?;
///     let upload_id = create_multipart_output.upload_id().unwrap();
///
///     // Create MultipartUploadContext
///     let mut upload_context = MultipartUploadContext {
///         s3_manager: s3_manager.clone(),
///         bucket: "my-bucket".to_string(),
///         key: "my-key".to_string(),
///         upload_id: upload_id.to_string(),
///         parts: Vec::new(),
///         total_size: 0,
///     };
///
///     // Upload parts
///     upload_context.upload_part(1, Bytes::from("Part 1 data")).await?;
///     upload_context.upload_part(2, Bytes::from("Part 2 data")).await?;
///
///     // Complete the multipart upload
///     let upload_result = upload_context.complete().await?;
///     println!("Upload completed. ETag: {}, Size: {}", upload_result.etag, upload_result.size);
///
///     Ok(())
/// }
/// ```
pub struct MultipartUploadContext {
    s3_manager: S3Manager,
    bucket: String,
    key: String,
    upload_id: String,
    parts: Vec<CompletedPart>,
    total_size: u64,
}

impl MultipartUploadContext {
    /// Uploads a part in the multipart upload.
    ///
    /// # Arguments
    ///
    /// * `part_number` - The number of the part being uploaded.
    /// * `body` - The data for this part.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn upload_part(&mut self, part_number: i32, body: Bytes) -> Result<()> {
        let completed_part = self.s3_manager.upload_part(
            &self.bucket,
            &self.key,
            &self.upload_id,
            part_number,
            body.to_vec()
        ).await?;

        self.parts.push(completed_part);
        self.total_size += body.len() as u64;

        Ok(())
    }
    /// Completes the multipart upload.
    ///
    /// # Returns
    ///
    /// A Result containing the UploadResult or an error.
    pub async fn complete(self) -> Result<UploadResult> {
        self.s3_manager.complete_multipart_upload(
            &self.bucket,
            &self.key,
            &self.upload_id,
            self.parts
        ).await?;

        Ok(UploadResult {
            etag: Ulid::new().into(), // В реальном приложении здесь должен быть фактический ETag
            size: self.total_size,
        })
    }
    /// Aborts the multipart upload.
    ///
    /// # Returns
    ///
    /// A Result containing the UploadResult or an error.
    pub async fn abort(self) -> Result<()> {
        self.s3_manager.abort_multipart_upload(
            &self.bucket,
            &self.key,
            &self.upload_id,
        ).await?;
        Ok(())
    }
}



/// Represents the result of a completed upload.
#[derive(Debug, Clone)]
pub struct UploadResult {
    pub etag: String,
    pub size: u64,
}
