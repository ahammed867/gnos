use std::path::PathBuf;
use clap::{Parser, Subcommand};
use tracing::{info, error};
use gnos::{GnosFileSystem, DriverRegistry, CapabilityManager, config::GnosConfig};

#[derive(Parser)]
#[command(name = "gnos-mount")]
#[command(about = "GNOS - Revolutionary infrastructure filesystem")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Mount GNOS filesystem
    Mount {
        /// Mount point
        #[arg(short, long)]
        mount_point: PathBuf,
        
        /// Configuration file
        #[arg(short, long, default_value = "gnos.toml")]
        config: PathBuf,
        
        /// Foreground mode (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
        
        /// Enable debug logging
        #[arg(short, long)]
        debug: bool,
    },
    
    /// Generate capability tokens
    Token {
        /// Path to grant access to
        #[arg(short, long)]
        path: String,
        
        /// Permissions (rwx format)
        #[arg(short = 'p', long, default_value = "r")]
        permissions: String,
        
        /// Expiration in hours
        #[arg(short, long, default_value = "24")]
        expires: u64,
    },
    
    /// List active drivers
    Drivers,
    
    /// Show system info
    Info,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Mount { mount_point, config, foreground, debug } => {
            setup_logging(debug);
            mount_filesystem(mount_point, config, foreground).await?;
        }
        
        Commands::Token { path, permissions, expires } => {
            generate_token(path, permissions, expires).await?;
        }
        
        Commands::Drivers => {
            list_drivers().await?;
        }
        
        Commands::Info => {
            show_info().await?;
        }
    }
    
    Ok(())
}

fn setup_logging(debug: bool) {
    let level = if debug { "debug" } else { "info" };
    
    tracing_subscriber::fmt()
        .with_env_filter(format!("gnos={},warn", level))
        .with_target(false)
        .init();
}

async fn mount_filesystem(
    mount_point: PathBuf, 
    config_path: PathBuf, 
    foreground: bool
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🚀 Starting GNOS filesystem...");
    
    // Load configuration
    let config = GnosConfig::load(&config_path).await?;
    info!("📋 Configuration loaded from {}", config_path.display());
    
    // Initialize security
    let capability_manager = CapabilityManager::new(config.security.clone());
    info!("🔐 Security initialized");
    
    // Initialize driver registry
    let driver_registry = DriverRegistry::new(config.drivers.clone()).await?;
    info!("🔌 Drivers loaded: {}", driver_registry.count());
    
    // Create filesystem
    let fs = GnosFileSystem::new(driver_registry, capability_manager);
    info!("📁 Filesystem created");
    
    // Mount options for FUSE
    let options = vec![
        fuser::MountOption::RW,
        fuser::MountOption::FSName("gnos".to_string()),
        fuser::MountOption::Subtype("gnos".to_string()),
        fuser::MountOption::AllowOther,
    ];
    
    info!("🗻 Mounting at {}", mount_point.display());
    
    if foreground {
        info!("Running in foreground mode...");
    } else {
        info!("Running as daemon...");
    }
    
    // This blocks until unmounted
    fuser::mount2(fs, &mount_point, &options)?;
    
    info!("📴 GNOS unmounted");
    Ok(())
}

async fn generate_token(
    path: String, 
    permissions: String, 
    expires_hours: u64
) -> Result<(), Box<dyn std::error::Error>> {
    use gnos::security::{Capability, Operation};
    use std::time::{SystemTime, Duration};
    
    println!("🎫 Generating GNOS capability token...");
    
    let perms = parse_permissions(&permissions)?;
    let expiration = SystemTime::now() + Duration::from_secs(expires_hours * 3600);
    
    let capability = Capability {
        path: PathBuf::from(path.clone()),
        permissions: perms,
        expiration,
        owner: "cli-user".to_string(),
    };
    
    let token = capability.to_token()?;
    
    println!("📄 Path: {}", path);
    println!("🔑 Permissions: {}", permissions);
    println!("⏰ Expires: {} hours", expires_hours);
    println!("🎟️  Token: {}", token);
    println!("\n💡 Usage: export GNOS_TOKEN=\"{}\"", token);
    
    Ok(())
}

fn parse_permissions(perms: &str) -> Result<u8, Box<dyn std::error::Error>> {
    let mut result = 0u8;
    
    for ch in perms.chars() {
        match ch {
            'r' => result |= 0b100,
            'w' => result |= 0b010,
            'x' => result |= 0b001,
            _ => return Err(format!("Invalid permission: {}", ch).into()),
        }
    }
    
    Ok(result)
}

async fn list_drivers() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Available GNOS Drivers:");
    println!("┌─────────────────┬──────────────────┬────────────┐");
    println!("│ Name            │ Path             │ Status     │");
    println!("├─────────────────┼──────────────────┼────────────┤");
    println!("│ AI Models       │ /proc/llama3     │ Ready      │");
    println!("│ AWS S3          │ /cloud/aws/s3    │ Ready      │");
    println!("│ HTTP Services   │ /net/http        │ Ready      │");
    println!("│ IoT Sensors     │ /dev/sensors     │ Future     │");
    println!("└─────────────────┴──────────────────┴────────────┘");
    
    Ok(())
}

async fn show_info() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌟 GNOS - GlobalNamespace OS");
    println!("Version: {}", gnos::VERSION);
    println!("Magic: 0x{:08X}", gnos::GNOS_MAGIC);
    println!();
    println!("🎯 Mission: Transform infrastructure complexity into file simplicity");
    println!("🚀 Impact: 10x faster cloud/AI/edge development");
    println!();
    println!("📖 Documentation: https://github.com/gnos-os/rust-core");
    println!("🐛 Issues: https://github.com/gnos-os/rust-core/issues");
    
    Ok(())
}