use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct GnosInode {
    pub ino: u64,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: u16,
    pub mtime: SystemTime,
    pub ctime: SystemTime,
    pub crtime: SystemTime,
}

impl GnosInode {
    pub fn new_directory(ino: u64, path: PathBuf) -> Self {
        let now = SystemTime::now();
        Self {
            ino,
            path,
            is_dir: true,
            size: 4096,
            permissions: 0o755,
            mtime: now,
            ctime: now,
            crtime: now,
        }
    }
    
    pub fn new_file(ino: u64, path: PathBuf) -> Self {
        let now = SystemTime::now();
        Self {
            ino,
            path,
            is_dir: false,
            size: 0,
            permissions: 0o644,
            mtime: now,
            ctime: now,
            crtime: now,
        }
    }
}

pub struct InodeManager {
    inodes: Arc<RwLock<HashMap<u64, GnosInode>>>,
    path_to_ino: Arc<RwLock<HashMap<PathBuf, u64>>>,
    next_ino: Arc<RwLock<u64>>,
}

impl InodeManager {
    pub fn new() -> Self {
        Self {
            inodes: Arc::new(RwLock::new(HashMap::new())),
            path_to_ino: Arc::new(RwLock::new(HashMap::new())),
            next_ino: Arc::new(RwLock::new(2)), // Start from 2 (1 is root)
        }
    }
    
    pub fn create_directory(&mut self, ino: u64, path: PathBuf) -> u64 {
        let inode = GnosInode::new_directory(ino, path.clone());
        
        self.inodes.write().unwrap().insert(ino, inode);
        self.path_to_ino.write().unwrap().insert(path, ino);
        
        ino
    }
    
    pub fn create_file(&mut self, ino: u64, path: PathBuf) -> u64 {
        let inode = GnosInode::new_file(ino, path.clone());
        
        self.inodes.write().unwrap().insert(ino, inode);
        self.path_to_ino.write().unwrap().insert(path, ino);
        
        ino
    }
    
    pub fn get(&self, ino: u64) -> Option<GnosInode> {
        self.inodes.read().unwrap().get(&ino).cloned()
    }
    
    pub fn find_by_path(&self, path: &PathBuf) -> Option<u64> {
        self.path_to_ino.read().unwrap().get(path).copied()
    }
}
