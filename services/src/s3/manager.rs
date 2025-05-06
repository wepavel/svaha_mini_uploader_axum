use std::path::Path;
use aws_sdk_s3::{Client, Config, config::{Credentials, Region, BehaviorVersion}};
use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::operation::put_object::PutObjectOutput;
use bytes::Bytes;
use tokio::io::AsyncReadExt;
use super::multipart::{MultipartUploadContext, MultipartUploadOptions};
use super::errors::{Result, S3Error};
use std::sync::Arc;

/// Менеджер для взаимодействия с S3 или совместимым объектным хранилищем
#[derive(Clone)]
pub struct S3Manager {
    // client: Client,
    client_factory: Arc<dyn Fn() -> Client + Send + Sync>,
}

impl S3Manager {
    /// Создает новый экземпляр S3Manager
    // pub async fn new(region: String, endpoint: Option<String>, credentials: Credentials) -> Result<Self> {
    //     let region = Region::new(region);
    //     let mut config_builder = Config::builder()
    //         .region(region)
    //         .credentials_provider(credentials)
    //         .behavior_version(BehaviorVersion::latest());
    // 
    //     if let Some(endpoint_url) = endpoint {
    //         config_builder = config_builder.endpoint_url(endpoint_url);
    //     }
    // 
    //     let config = config_builder.build();
    //     let client = Client::from_conf(config);
    // 
    //     Ok(Self { client })
    // }
    // 
    // /// Получает клиент AWS S3
    // pub fn client(&self) -> &Client {
    //     &self.client
    // }
    pub async fn new(region: String, endpoint: Option<String>, credentials: Credentials) -> Result<Self> {
        let region = Region::new(region);

        // Сохраняем параметры настройки
        let region_clone = region.clone();
        let endpoint_clone = endpoint.clone();
        let credentials_clone = credentials.clone();

        // Создаем фабрику клиентов вместо одного клиента
        let client_factory = Arc::new(move || {
            let mut config_builder = Config::builder()
                .region(region_clone.clone())
                .credentials_provider(credentials_clone.clone())
                .behavior_version(BehaviorVersion::latest());

            if let Some(endpoint_url) = &endpoint_clone {
                config_builder = config_builder.endpoint_url(endpoint_url.clone());
            }

            let config = config_builder.build();
            Client::from_conf(config)
        });

        Ok(Self { client_factory })
    }

    // Получаем новый клиент для каждой операции
    fn get_client(&self) -> Client {
        (self.client_factory)()
    }


    /// Загружает объект в S3 из массива байтов
    pub async fn put_object(&self, bucket: &str, key: &str, data: Bytes) -> Result<PutObjectOutput> {
        Ok(self.get_client()
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(data.into())
            .send()
            .await?)
    }

    /// Скачивает объект из S3
    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<GetObjectOutput> {
        Ok(self.get_client()
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?)
    }

    /// Скачивает объект из S3 в виде байтов
    pub async fn get_object_bytes(&self, bucket: &str, key: &str) -> Result<Bytes> {
        let response = self.get_object(bucket, key).await?;
        let mut body = response.body.into_async_read();
        let mut buffer = Vec::new();
        body.read_to_end(&mut buffer).await?;
        Ok(Bytes::from(buffer))
    }

    /// Копирует объект из одного бакета в другой
    pub async fn copy_object(
        &self,
        source_bucket: &str,
        destination_bucket: &str,
        source_object: &str,
        destination_object: &str,
    ) -> Result<()> {
        let source_key = format!("{source_bucket}/{source_object}");
        let response = self.get_client()
            .copy_object()
            .copy_source(&source_key)
            .bucket(destination_bucket)
            .key(destination_object)
            .send()
            .await?;

        tracing::info!(
            "Copied from {source_key} to {destination_bucket}/{destination_object}"
        );
        Ok(())
    }

    /// Удаляет объект из бакета
    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        self.get_client()
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }

    // ОПЕРАЦИИ С ФАЙЛАМИ

    /// Загружает файл в S3 из локального пути
    pub async fn upload_file(&self, bucket: &str, local_path: &str, key: &str) -> Result<PutObjectOutput> {
        let body = aws_sdk_s3::primitives::ByteStream::from_path(Path::new(local_path))
            .await
            .map_err(|e| S3Error::UploadError(format!("Failed to read file {}: {}", local_path, e)))?;

        Ok(self.get_client()
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .send()
            .await?)
    }

    /// Скачивает файл из S3 в локальный путь
    pub async fn download_file(&self, bucket: &str, key: &str, local_path: &str) -> Result<()> {
        let response = self.get_object(bucket, key).await?;
        let mut body = response.body.into_async_read();

        let mut file = tokio::fs::File::create(local_path).await?;
        tokio::io::copy(&mut body, &mut file).await?;

        Ok(())
    }

    // ОПЕРАЦИИ С МУЛЬТИЧАСТНОЙ ЗАГРУЗКОЙ

    /// Создает контекст для многочастной загрузки
    pub async fn create_multipart_upload_context(
        &self,
        bucket: &str,
        key: &str,
        options: Option<MultipartUploadOptions>
    ) -> Result<MultipartUploadContext> {
        MultipartUploadContext::new(self.get_client(), bucket, key, options).await
    }

    /// Высокоуровневый метод для загрузки больших данных с автоматическим
    /// разделением на части нужного размера
    pub async fn upload_large_object(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        options: Option<MultipartUploadOptions>
    ) -> Result<()> {
        let options = options.unwrap_or_default();
        let chunk_size = options.chunk_size;

        // Если данные меньше размера чанка, используем обычную загрузку
        if data.len() <= chunk_size {
            self.put_object(bucket, key, data).await?;
            return Ok(());
        }

        // Иначе используем многочастную загрузку
        let mut context = self.create_multipart_upload_context(bucket, key, Some(options)).await?;

        let mut part_number = 1;
        let mut offset = 0;

        while offset < data.len() {
            let end = std::cmp::min(offset + chunk_size, data.len());
            let chunk = data.slice(offset..end);

            context.upload_part(part_number, chunk).await?;

            part_number += 1;
            offset = end;
        }

        context.complete().await?;
        Ok(())
    }

    // ПРОВЕРКИ И УТИЛИТЫ

    /// Проверяет существование объекта в бакете
    pub async fn is_file(&self, bucket: &str, key: &str) -> Result<bool> {
        match self.get_client()
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                if let aws_sdk_s3::error::SdkError::ServiceError(service_error) = &err {
                    if service_error.err().is_not_found() {
                        return Ok(false);
                    }
                }
                Err(err.into())
            }
        }
    }

    /// Проверяет существование файла с любым из указанных расширений
    pub async fn is_file_any_extension(
        &self,
        bucket: &str,
        key_prefix: &str,
        extensions: &[String]
    ) -> Result<bool> {
        // Сначала проверяем файл без расширения
        if self.is_file(bucket, key_prefix).await? {
            return Ok(true);
        }

        // Затем проверяем все предоставленные расширения
        for ext in extensions {
            let key = format!("{}{}", key_prefix, ext);
            if self.is_file(bucket, &key).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Перечисляет объекты в бакете
    pub async fn list_objects(&self, bucket: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut paginator = self.get_client()
            .list_objects_v2()
            .bucket(bucket)
            .into_paginator()
            .send();

        while let Some(result) = paginator.next().await {
            match result {
                Ok(output) => {
                    for object in output.contents() {
                        if let Some(key) = object.key() {
                            keys.push(key.to_string());
                            tracing::info!(" - {}", key);
                        }
                    }
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        Ok(keys)
    }

    /// Удаляет несколько объектов из бакета
    pub async fn delete_objects(&self, bucket: &str, objects_to_delete: Vec<String>) -> Result<()> {
        if objects_to_delete.is_empty() {
            return Ok(());
        }

        // Создаем список ObjectIdentifier из ключей
        let delete_object_ids: Vec<ObjectIdentifier> = objects_to_delete
            .into_iter()
            .map(|obj| {
                ObjectIdentifier::builder()
                    .key(obj)
                    .build()
                    .expect("Failed to build ObjectIdentifier")
            })
            .collect();

        // Создаем структуру Delete для массового удаления
        let delete = Delete::builder()
            .set_objects(Some(delete_object_ids))
            .build()
            .map_err(|err| S3Error::Other(format!("Failed to build delete_object input: {}", err)))?;

        // Выполняем удаление
        self.get_client()
            .delete_objects()
            .bucket(bucket)
            .delete(delete)
            .send()
            .await?;

        Ok(())
    }

    /// Очищает все объекты из бакета
    pub async fn clear_bucket(&self, bucket: &str) -> Result<Vec<String>> {
        let keys = self.list_objects(bucket).await?;

        if keys.is_empty() {
            return Ok(vec![]);
        }

        self.delete_objects(bucket, keys.clone()).await?;

        // Проверяем, что бакет действительно пуст
        let remaining = self.list_objects(bucket).await?;
        if remaining.is_empty() {
            Ok(keys)
        } else {
            Err(S3Error::Other("Failed to clear all objects from bucket".into()))
        }
    }

    /// Создает бакет, если он не существует
    pub async fn ensure_bucket_exists(&self, bucket: &str) -> Result<()> {
        // Проверяем существование бакета
        match self.get_client().head_bucket().bucket(bucket).send().await {
            Ok(_) => return Ok(()), // Бакет существует
            Err(err) => {
                // Проверяем, является ли ошибка "бакет не найден"
                if let aws_sdk_s3::error::SdkError::ServiceError(service_error) = &err {
                    if !service_error.err().is_not_found() {
                        return Err(err.into()); // Другая ошибка
                    }
                } else {
                    return Err(err.into()); // Другая ошибка
                }
            }
        }

        // Создаем бакет, если он не существует
        self.get_client()
            .create_bucket()
            .bucket(bucket)
            .send()
            .await?;

        Ok(())
    }
}