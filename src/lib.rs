//! GNOS - GlobalNamespace OS
//! 
//! Revolutionary POSIX filesystem interface for all computing resources.
//! Transforms cloud services, AI models, and APIs into simple file operations.

pub mod config;
pub mod drivers;
pub mod security;
pub mod vfs;

// Re-export core types
pub use drivers::{GnosDriver, DriverRegistry};
pub use security::{Capability, CapabilityManager, Operation};
pub use vfs::{GnosFileSystem, InodeManager};

// Core error types
pub type Result<T> = std::result::Result<T, GnosError>;

#[derive(Debug, thiserror::Error)]
pub enum GnosError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Path not found: {0}")]
    PathNotFound(String),
    
    #[error("Driver error: {0}")]
    Driver(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Capability expired")]
    CapabilityExpired,
    
    #[error("Invalid path format: {0}")]
    InvalidPath(String),
    
    #[error("Resource busy: {0}")]
    ResourceBusy(String),
}

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GNOS_MAGIC: u64 = 0x474E4F53; // "GNOS" in hex