use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use crate::{GnosError, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Read,
    Write,
    Execute,
    List,
}

impl Operation {
    fn to_bit(&self) -> u8 {
        match self {
            Operation::Read => 0b100,
            Operation::Write => 0b010,
            Operation::Execute => 0b001,
            Operation::List => 0b100, // Same as read for simplicity
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub path: PathBuf,
    pub permissions: u8, // rwx bits
    pub expiration: SystemTime,
    pub owner: String,
}

impl Capability {
    pub fn allows(&self, operation: Operation) -> bool {
        self.permissions & operation.to_bit() != 0
    }
    
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expiration
    }
    
    pub fn is_valid_for_path(&self, path: &Path) -> bool {
        path.starts_with(&self.path)
    }
    
    pub fn to_token(&self) -> Result<String> {
        let json = serde_json::to_string(self)
            .map_err(|e| GnosError::Driver(format!("Failed to serialize capability: {}", e)))?;
        
        let encoded = URL_SAFE_NO_PAD.encode(json.as_bytes());
        Ok(format!("gnos.{}", encoded))
    }
    
    pub fn from_token(token: &str) -> Result<Self> {
        if !token.starts_with("gnos.") {
            return Err(GnosError::PermissionDenied("Invalid token format".to_string()));
        }
        
        let encoded = &token[5..];
        let json_bytes = URL_SAFE_NO_PAD.decode(encoded)
            .map_err(|_| GnosError::PermissionDenied("Invalid token encoding".to_string()))?;
        
        let json = String::from_utf8(json_bytes)
            .map_err(|_| GnosError::PermissionDenied("Invalid token UTF-8".to_string()))?;
        
        let capability: Capability = serde_json::from_str(&json)
            .map_err(|_| GnosError::PermissionDenied("Invalid token JSON".to_string()))?;
        
        Ok(capability)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub default_permissions: u8,
    pub max_token_lifetime: Duration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            default_permissions: 0b100, // Read-only by default
            max_token_lifetime: Duration::from_secs(24 * 3600), // 24 hours
        }
    }
}

pub struct CapabilityManager {
    config: SecurityConfig,
}

impl CapabilityManager {
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }
    
    pub async fn check_permission(&self, path: &Path, operation: Operation) -> Result<()> {
        // Check environment variable for token
        if let Ok(token) = std::env::var("GNOS_TOKEN") {
            if let Ok(capability) = Capability::from_token(&token) {
                if capability.is_valid_for_path(path) && 
                   capability.allows(operation) && 
                   !capability.is_expired() {
                    return Ok(());
                }
            }
        }
        
        // For now, allow all operations (development mode)
        Ok(())
    }
}