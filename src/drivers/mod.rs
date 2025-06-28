pub mod traits;
pub mod ai;
pub mod cloud;
pub mod http;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

pub use traits::{GnosDriver, ResourceMetadata};
use crate::config::DriverConfig;
use crate::Result;

pub struct DriverRegistry {
    drivers: HashMap<String, Arc<dyn GnosDriver>>,
}

impl DriverRegistry {
    pub async fn new(config: DriverConfig) -> Result<Self> {
        let mut drivers: HashMap<String, Arc<dyn GnosDriver>> = HashMap::new();
        
        info!("🔌 Initializing GNOS drivers...");
        
        // Initialize AI driver
        if config.ai.enabled {
            match ai::AiDriver::new().await {
                Ok(driver) => {
                    info!("✅ AI driver initialized");
                    drivers.insert("ai".to_string(), Arc::new(driver));
                }
                Err(e) => {
                    warn!("❌ Failed to initialize AI driver: {}", e);
                }
            }
        }
        
        // Initialize Cloud driver
        if config.cloud.enabled {
            match cloud::CloudDriver::new().await {
                Ok(driver) => {
                    info!("✅ Cloud driver initialized");
                    drivers.insert("cloud".to_string(), Arc::new(driver));
                }
                Err(e) => {
                    warn!("❌ Failed to initialize Cloud driver: {}", e);
                }
            }
        }
        
        // Initialize HTTP driver
        if config.http.enabled {
            match http::HttpDriver::new().await {
                Ok(driver) => {
                    info!("✅ HTTP driver initialized");
                    drivers.insert("http".to_string(), Arc::new(driver));
                }
                Err(e) => {
                    warn!("❌ Failed to initialize HTTP driver: {}", e);
                }
            }
        }
        
        info!("🎯 Driver registry initialized with {} drivers", drivers.len());
        
        Ok(Self { drivers })
    }
    
    pub fn get_driver(&self, path: &Path) -> Option<Arc<dyn GnosDriver>> {
        // Find the best matching driver for this path
        for driver in self.drivers.values() {
            if driver.supports(path) {
                return Some(driver.clone());
            }
        }
        None
    }
    
    pub fn count(&self) -> usize {
        self.drivers.len()
    }
}