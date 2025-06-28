use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::security::SecurityConfig;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnosConfig {
    pub security: SecurityConfig,
    pub drivers: DriverConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverConfig {
    pub ai: AiDriverConfig,
    pub cloud: CloudDriverConfig,
    pub http: HttpDriverConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDriverConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDriverConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpDriverConfig {
    pub enabled: bool,
}

impl Default for GnosConfig {
    fn default() -> Self {
        Self {
            security: SecurityConfig::default(),
            drivers: DriverConfig::default(),
        }
    }
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            ai: AiDriverConfig::default(),
            cloud: CloudDriverConfig::default(),
            http: HttpDriverConfig::default(),
        }
    }
}

impl Default for AiDriverConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for CloudDriverConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for HttpDriverConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl GnosConfig {
    pub async fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = tokio::fs::read_to_string(path).await?;
            let config: GnosConfig = toml::from_str(&content)
                .map_err(|e| crate::GnosError::Driver(format!("Invalid config: {}", e)))?;
            Ok(config)
        } else {
            // Create default config
            let config = Self::default();
            config.save(path).await?;
            Ok(config)
        }
    }
    
    pub async fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::GnosError::Driver(format!("Failed to serialize config: {}", e)))?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }
}