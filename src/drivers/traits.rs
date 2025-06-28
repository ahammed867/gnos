use std::path::Path;
use async_trait::async_trait;
use crate::Result;

/// Core driver trait - every resource type implements this
#[async_trait]
pub trait GnosDriver: Send + Sync {
    /// Read data from the resource
    async fn read(&self, path: &Path) -> Result<Vec<u8>>;
    
    /// Write data to the resource
    async fn write(&self, path: &Path, data: &[u8]) -> Result<()>;
    
    /// List resources (for directory-like resources)
    async fn list(&self, path: &Path) -> Result<Vec<String>>;
    
    /// Check if resource exists
    async fn exists(&self, path: &Path) -> Result<bool>;
    
    /// Get resource metadata
    async fn metadata(&self, path: &Path) -> Result<ResourceMetadata>;
    
    /// Driver name for identification
    fn name(&self) -> &'static str;
    
    /// Supported path patterns
    fn supports(&self, path: &Path) -> bool;
}

#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    pub size: u64,
    pub is_directory: bool,
    pub last_modified: std::time::SystemTime,
    pub mime_type: Option<String>,
    pub custom_fields: std::collections::HashMap<String, String>,
}

impl Default for ResourceMetadata {
    fn default() -> Self {
        Self {
            size: 0,
            is_directory: false,
            last_modified: std::time::SystemTime::now(),
            mime_type: None,
            custom_fields: std::collections::HashMap::new(),
        }
    }
}