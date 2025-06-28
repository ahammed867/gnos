use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use ring::{digest, hmac};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use tracing::{debug, info, warn};

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
    pub issued_at: SystemTime,
    pub signature: Option<String>,
}

impl Capability {
    pub fn new(
        path: PathBuf,
        permissions: u8,
        owner: String,
        duration: Duration,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            path,
            permissions,
            expiration: now + duration,
            owner,
            issued_at: now,
            signature: None,
        }
    }
    
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
    
    pub fn sign(&mut self, secret: &[u8]) -> Result<()> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let data = format!("{}:{}:{}:{}", 
                          self.path.display(), 
                          self.permissions, 
                          self.expiration.duration_since(SystemTime::UNIX_EPOCH)
                              .unwrap_or_default().as_secs(),
                          self.owner);
        
        let tag = hmac::sign(&key, data.as_bytes());
        self.signature = Some(URL_SAFE_NO_PAD.encode(tag.as_ref()));
        
        Ok(())
    }
    
    pub fn verify(&self, secret: &[u8]) -> bool {
        let Some(ref signature) = self.signature else {
            return false;
        };
        
        let Ok(signature_bytes) = URL_SAFE_NO_PAD.decode(signature) else {
            return false;
        };
        
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let data = format!("{}:{}:{}:{}", 
                          self.path.display(), 
                          self.permissions, 
                          self.expiration.duration_since(SystemTime::UNIX_EPOCH)
                              .unwrap_or_default().as_secs(),
                          self.owner);
        
        hmac::verify(&key, data.as_bytes(), &signature_bytes).is_ok()
    }
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub default_permissions: u8,
    pub max_token_lifetime: Duration,
    pub require_signatures: bool,
    pub hmac_secret: Vec<u8>,
    pub trusted_issuers: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        // Generate a random secret in production
        let mut secret = vec![0u8; 32];
        secret.copy_from_slice(b"gnos-dev-secret-change-in-prod!!");
        
        Self {
            default_permissions: 0b100, // Read-only by default
            max_token_lifetime: Duration::from_secs(24 * 3600), // 24 hours
            require_signatures: true,
            hmac_secret: secret,
            trusted_issuers: vec!["gnos-cli".to_string(), "gnos-web".to_string()],
        }
    }
}

pub struct CapabilityManager {
    config: SecurityConfig,
    active_capabilities: Arc<RwLock<HashMap<String, Capability>>>,
    capability_cache: Arc<RwLock<HashMap<String, (Capability, SystemTime)>>>,
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
}

#[derive(Debug, Clone)]
struct AuditEntry {
    timestamp: SystemTime,
    operation: Operation,
    path: PathBuf,
    owner: String,
    success: bool,
    reason: Option<String>,
}

impl CapabilityManager {
    pub fn new(config: SecurityConfig) -> Self {
        info!("🔐 Initializing GNOS security system");
        
        Self {
            config,
            active_capabilities: Arc::new(RwLock::new(HashMap::new())),
            capability_cache: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub async fn check_permission(&self, path: &Path, operation: Operation) -> Result<()> {
        debug!("🔍 Checking permission: {} for {:?}", path.display(), operation);
        
        // Check environment variable for token
        if let Ok(token) = std::env::var("GNOS_TOKEN") {
            if let Ok(capability) = self.validate_token(&token).await {
                if capability.is_valid_for_path(path) && capability.allows(operation) {
                    self.log_access(path, operation, &capability.owner, true, None).await;
                    return Ok(());
                }
            }
        }
        
        // Check active capabilities
        let capabilities = self.active_capabilities.read().await;
        for capability in capabilities.values() {
            if capability.is_valid_for_path(path) && 
               capability.allows(operation) && 
               !capability.is_expired() {
                self.log_access(path, operation, &capability.owner, true, None).await;
                return Ok(());
            }
        }
        
        // Default deny with audit
        let reason = "No valid capability found".to_string();
        self.log_access(path, operation, "unknown", false, Some(reason.clone())).await;
        
        Err(GnosError::PermissionDenied(format!(
            "Access denied to {} for {:?}: {}", 
            path.display(), 
            operation, 
            reason
        )))
    }
    
    pub async fn grant_capability(
        &self,
        path: PathBuf,
        permissions: u8,
        owner: String,
        duration: Duration,
    ) -> Result<String> {
        // Enforce maximum lifetime
        let duration = std::cmp::min(duration, self.config.max_token_lifetime);
        
        let mut capability = Capability::new(path, permissions, owner, duration);
        
        // Sign the capability if required
        if self.config.require_signatures {
            capability.sign(&self.config.hmac_secret)?;
        }
        
        let token = capability.to_token()?;
        let capability_id = self.hash_capability(&capability);
        
        // Store in active capabilities
        self.active_capabilities.write().await
            .insert(capability_id, capability.clone());
        
        info!("✅ Granted capability: {} -> {}", capability.owner, capability.path.display());
        
        Ok(token)
    }
    
    pub async fn revoke_capability(&self, token: &str) -> Result<()> {
        let capability = Capability::from_token(token)?;
        let capability_id = self.hash_capability(&capability);
        
        self.active_capabilities.write().await.remove(&capability_id);
        self.capability_cache.write().await.remove(&capability_id);
        
        info!("🚫 Revoked capability: {} -> {}", capability.owner, capability.path.display());
        
        Ok(())
    }
    
    async fn validate_token(&self, token: &str) -> Result<Capability> {
        // Check cache first
        let cache_key = token.to_string();
        {
            let cache = self.capability_cache.read().await;
            if let Some((capability, cached_at)) = cache.get(&cache_key) {
                // Cache for 60 seconds
                if cached_at.elapsed().unwrap_or_default() < Duration::from_secs(60) {
                    if !capability.is_expired() {
                        return Ok(capability.clone());
                    }
                }
            }
        }
        
        // Parse and validate token
        let capability = Capability::from_token(token)?;
        
        // Check expiration
        if capability.is_expired() {
            return Err(GnosError::CapabilityExpired);
        }
        
        // Verify signature if required
        if self.config.require_signatures {
            if !capability.verify(&self.config.hmac_secret) {
                return Err(GnosError::PermissionDenied("Invalid signature".to_string()));
            }
        }
        
        // Cache the validated capability
        self.capability_cache.write().await
            .insert(cache_key, (capability.clone(), SystemTime::now()));
        
        Ok(capability)
    }
    
    fn hash_capability(&self, capability: &Capability) -> String {
        let data = format!("{}:{}:{}:{}", 
                          capability.path.display(),
                          capability.permissions,
                          capability.owner,
                          capability.issued_at.duration_since(SystemTime::UNIX_EPOCH)
                              .unwrap_or_default().as_secs());
        
        let hash = digest::digest(&digest::SHA256, data.as_bytes());
        URL_SAFE_NO_PAD.encode(hash.as_ref())
    }
    
    async fn log_access(
        &self,
        path: &Path,
        operation: Operation,
        owner: &str,
        success: bool,
        reason: Option<String>,
    ) {
        let entry = AuditEntry {
            timestamp: SystemTime::now(),
            operation,
            path: path.to_path_buf(),
            owner: owner.to_string(),
            success,
            reason,
        };
        
        self.audit_log.write().await.push(entry);
        
        // Keep only last 10,000 entries
        let mut log = self.audit_log.write().await;
        if log.len() > 10_000 {
            log.drain(0..5_000);
        }
    }
    
    pub async fn get_audit_log(&self) -> Vec<AuditEntry> {
        self.audit_log.read().await.clone()
    }
    
    pub async fn cleanup_expired(&self) {
        let now = SystemTime::now();
        
        // Clean active capabilities
        {
            let mut capabilities = self.active_capabilities.write().await;
            capabilities.retain(|_, cap| !cap.is_expired());
        }
        
        // Clean cache
        {
            let mut cache = self.capability_cache.write().await;
            cache.retain(|_, (cap, cached_at)| {
                !cap.is_expired() && cached_at.elapsed().unwrap_or_default() < Duration::from_secs(300)
            });
        }
        
        debug!("🧹 Cleaned up expired capabilities");
    }
    
    pub async fn get_stats(&self) -> CapabilityStats {
        let capabilities = self.active_capabilities.read().await;
        let cache = self.capability_cache.read().await;
        let audit_log = self.audit_log.read().await;
        
        let successful_accesses = audit_log.iter().filter(|e| e.success).count();
        let failed_accesses = audit_log.iter().filter(|e| !e.success).count();
        
        CapabilityStats {
            active_capabilities: capabilities.len(),
            cached_capabilities: cache.len(),
            total_audit_entries: audit_log.len(),
            successful_accesses,
            failed_accesses,
        }
    }
}

#[derive(Debug)]
pub struct CapabilityStats {
    pub active_capabilities: usize,
    pub cached_capabilities: usize,
    pub total_audit_entries: usize,
    pub successful_accesses: usize,
    pub failed_accesses: usize,
}

// Periodic cleanup task
pub async fn start_cleanup_task(capability_manager: Arc<CapabilityManager>) {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
    
    loop {
        interval.tick().await;
        capability_manager.cleanup_expired().await;
    }
}