#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Upload failed: {0}")]
    Upload(String),
    #[error("Config error: {0}")]
    Config(String),
}

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    /// Upload file and return the public URL.
    async fn upload(
        &self,
        key: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<String, StorageError>;

    /// Get the public URL for a stored object.
    fn public_url(&self, key: &str) -> String;

    /// Download file and return the bytes.
    async fn download(&self, key: &str) -> Result<Vec<u8>, StorageError>;

    /// Check if an object exists in storage (HEAD request).
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;

    /// Delete an object. Returns Ok(()) even if the object did not exist (idempotent).
    async fn delete(&self, key: &str) -> Result<(), StorageError>;

    /// Extract the object key from a public URL.
    fn extract_key(&self, url: &str) -> Option<String>;

    /// Bucket name.
    fn bucket(&self) -> &str;
}
