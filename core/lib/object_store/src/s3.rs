//! S3-based [`ObjectStore`] implementation.
use async_trait::async_trait;
use aws_sdk_s3::{
    error::SdkError,
    operation::{
        delete_object::DeleteObjectError, get_object::GetObjectError, put_object::PutObjectError,
    },
    primitives::{ByteStream, ByteStreamError},
    Client,
};

use std::{fmt, future::Future, time::Instant};

use crate::raw::{Bucket, ObjectStore, ObjectStoreError};

pub struct S3Storage {
    bucket_url: String,
    client: Client,
}

impl fmt::Debug for S3Storage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("S3Storage")
            .field("bucket_url", &self.bucket_url)
            .finish_non_exhaustive()
    }
}

impl S3Storage {
    pub async fn new(endpoint_url: String, bucket_url: String) -> Self {
        let shared_config = aws_config::load_from_env().await;
        let config = aws_sdk_s3::config::Builder::from(&shared_config)
            .endpoint_url(endpoint_url)
            .build();

        let client = Client::from_conf(config);
        Self { client, bucket_url }
    }

    fn filename(bucket: &str, filename: &str) -> String {
        format!("{bucket}/{filename}")
    }

    pub async fn get_async(
        &self,
        bucket: &'static str,
        key: &str,
    ) -> Result<Vec<u8>, ObjectStoreError> {
        let started_at = Instant::now();
        let filename = Self::filename(bucket, key);
        tracing::trace!(
            "Fetching data from S3 for key {filename} from bucket {}",
            self.bucket_url
        );

        let object = self
            .client
            .get_object()
            .bucket(self.bucket_url.clone())
            .key(filename.clone())
            .send()
            .await?;

        tracing::trace!(
            "Fetched data from S3 for key {filename} from bucket {} and it took: {:?}",
            self.bucket_url,
            started_at.elapsed()
        );

        object
            .body
            .collect()
            .await
            .map(|data| data.to_vec())
            .map_err(ObjectStoreError::from)
    }

    pub async fn put_async(
        &self,
        bucket: &'static str,
        key: &str,
        value: Vec<u8>,
    ) -> Result<(), ObjectStoreError> {
        let started_at = Instant::now();
        let filename = Self::filename(bucket, key);
        tracing::trace!(
            "Storing data to s3 for key {filename} from bucket {}",
            self.bucket_url
        );

        let body = ByteStream::from(value);

        let result = self
            .client
            .put_object()
            .bucket(self.bucket_url.clone())
            .key(filename.clone())
            .body(body)
            .send()
            .await?;

        tracing::trace!("PutObjectOutput {:?}", result);

        tracing::trace!(
            "Stored data to S3 for key {filename} from bucket {} and it took: {:?}",
            self.bucket_url,
            started_at.elapsed()
        );

        Ok(())
    }

    // For some bizzare reason, `async fn` doesn't work here, failing with the following error:
    //
    // > hidden type for `impl std::future::Future<Output = Result<(), ObjectStoreError>>`
    // > captures lifetime that does not appear in bounds
    pub fn remove_async(
        &self,
        bucket: &'static str,
        key: &str,
    ) -> impl Future<Output = Result<(), ObjectStoreError>> + '_ {
        let filename = Self::filename(bucket, key);
        tracing::trace!(
            "Removing data from S3 for key {filename} from bucket {}",
            self.bucket_url
        );

        let request = self
            .client
            .delete_object()
            .bucket(self.bucket_url.clone())
            .key(filename.clone());

        async move {
            match request.send().await {
                Ok(_s) => Ok(()),
                Err(error) => return Err(ObjectStoreError::Other(error.into())),
            }
        }
    }
}

impl From<SdkError<GetObjectError>> for ObjectStoreError {
    fn from(sdk_err: SdkError<GetObjectError>) -> Self {
        let err = sdk_err.into_service_error();
        match err {
            GetObjectError::InvalidObjectState(_) => ObjectStoreError::KeyNotFound(err.into()),
            GetObjectError::NoSuchKey(_) => ObjectStoreError::KeyNotFound(err.into()),
            GetObjectError::Unhandled(_) => ObjectStoreError::Other(err.into()),
            _ => todo!(),
        }
    }
}

impl From<SdkError<PutObjectError>> for ObjectStoreError {
    fn from(sdk_err: SdkError<PutObjectError>) -> Self {
        let err = sdk_err.into_service_error();
        match err {
            PutObjectError::Unhandled(_) => ObjectStoreError::Other(err.into()),
            _ => todo!(),
        }
    }
}

impl From<SdkError<DeleteObjectError>> for ObjectStoreError {
    fn from(sdk_err: SdkError<DeleteObjectError>) -> Self {
        let err = sdk_err.into_service_error();
        match err {
            DeleteObjectError::Unhandled(_) => ObjectStoreError::Other(err.into()),
            _ => todo!(),
        }
    }
}

impl From<ByteStreamError> for ObjectStoreError {
    fn from(byte_stream_err: ByteStreamError) -> Self {
        ObjectStoreError::Other(byte_stream_err.into())
    }
}

#[async_trait]
impl ObjectStore for S3Storage {
    async fn get_raw(&self, bucket: Bucket, key: &str) -> Result<Vec<u8>, ObjectStoreError> {
        self.get_async(bucket.as_str(), key).await
    }

    async fn put_raw(
        &self,
        bucket: Bucket,
        key: &str,
        value: Vec<u8>,
    ) -> Result<(), ObjectStoreError> {
        self.put_async(bucket.as_str(), key, value).await
    }

    async fn remove_raw(&self, bucket: Bucket, key: &str) -> Result<(), ObjectStoreError> {
        self.remove_async(bucket.as_str(), key).await
    }
}

//#[cfg(test)]
// mod test {

//     use super::*;

//     // async fn test_blocking() {
//     //     let handle = Handle::current();
//     //     let result = S3Storage::block_on(&handle, async { 42 });
//     //     assert_eq!(result, 42);

//     //     let result =
//     //         tokio::task::spawn_blocking(move || S3Storage::block_on(&handle, async { 42 }));
//     //     assert_eq!(result.await.unwrap(), 42);
//     // }

//     #[tokio::test]
//     async fn blocking_in_sync_and_async_context() {
//         test_blocking().await;
//     }

//     #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
//     async fn blocking_in_sync_and_async_context_in_multithreaded_rt() {
//         test_blocking().await;
//     }
// }
