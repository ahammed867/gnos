use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, 
    ReplyEntry, ReplyWrite, ReplyOpen, Request,
};
use tracing::{debug, info};

use crate::drivers::DriverRegistry;
use crate::security::{CapabilityManager, Operation};
use crate::vfs::inode::{InodeManager, GnosInode};
use crate::{GnosError, Result};

const TTL: Duration = Duration::from_secs(1);
const ROOT_INODE: u64 = 1;

pub struct GnosFileSystem {
    driver_registry: DriverRegistry,
    capability_manager: CapabilityManager,
    inode_manager: InodeManager,
    open_files: HashMap<u64, OpenFile>,
    next_fh: u64,
}

#[derive(Debug)]
struct OpenFile {
    path: PathBuf,
    data: Option<Vec<u8>>,
}

impl GnosFileSystem {
    pub fn new(
        driver_registry: DriverRegistry,
        capability_manager: CapabilityManager,
    ) -> Self {
        let mut inode_manager = InodeManager::new();
        
        // Create root directory
        inode_manager.create_directory(ROOT_INODE, PathBuf::from("/"));
        
        // Pre-create known structure
        inode_manager.create_directory(2, PathBuf::from("/proc"));
        inode_manager.create_directory(3, PathBuf::from("/cloud"));
        inode_manager.create_directory(4, PathBuf::from("/net"));
        inode_manager.create_directory(5, PathBuf::from("/dev"));
        
        // AI models
        inode_manager.create_file(10, PathBuf::from("/proc/llama3"));
        
        Self {
            driver_registry,
            capability_manager,
            inode_manager,
            open_files: HashMap::new(),
            next_fh: 1,
        }
    }
    
    fn get_file_attr(&self, ino: u64) -> Result<FileAttr> {
        let inode = self.inode_manager.get(ino)
            .ok_or_else(|| GnosError::PathNotFound(format!("inode {}", ino)))?;
        
        let now = SystemTime::now();
        
        Ok(FileAttr {
            ino,
            size: inode.size,
            blocks: (inode.size + 511) / 512,
            atime: now,
            mtime: inode.mtime,
            ctime: inode.ctime,
            crtime: inode.crtime,
            kind: if inode.is_dir { FileType::Directory } else { FileType::RegularFile },
            perm: inode.permissions,
            nlink: if inode.is_dir { 2 } else { 1 },
            uid: 1000,
            gid: 1000,
            rdev: 0,
            flags: 0,
            blksize: 4096,
        })
    }
}

impl Filesystem for GnosFileSystem {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup: parent={}, name={:?}", parent, name);
        
        let parent_inode = match self.inode_manager.get(parent) {
            Some(inode) => inode,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        let child_path = parent_inode.path.join(name);
        
        if let Some(child_ino) = self.inode_manager.find_by_path(&child_path) {
            match self.get_file_attr(child_ino) {
                Ok(attr) => reply.entry(&TTL, &attr, 0),
                Err(_) => reply.error(libc::EIO),
            }
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr: ino={}", ino);
        
        match self.get_file_attr(ino) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(_) => reply.error(libc::ENOENT),
        }
    }
    
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        debug!("readdir: ino={}, offset={}", ino, offset);
        
        let entries = match ino {
            1 => vec![("proc", 2), ("cloud", 3), ("net", 4), ("dev", 5)], // root
            2 => vec![("llama3", 10)], // /proc
            3 => vec![("aws", 20), ("gcp", 21), ("azure", 22)], // /cloud
            4 => vec![("http", 30)], // /net
            5 => vec![("sensors", 40)], // /dev
            _ => vec![],
        };
        
        for (i, (name, ino)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*ino, (i + 1) as i64, FileType::RegularFile, name) {
                break;
            }
        }
        
        reply.ok();
    }
    
    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: ReplyOpen) {
        debug!("open: ino={}", ino);
        
        let inode = match self.inode_manager.get(ino) {
            Some(inode) if !inode.is_dir => inode,
            _ => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        let fh = self.next_fh;
        self.next_fh += 1;
        
        self.open_files.insert(fh, OpenFile {
            path: inode.path.clone(),
            data: None,
        });
        
        reply.opened(fh, 0);
    }
    
    fn read(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        debug!("read: fh={}, offset={}, size={}", fh, offset, size);
        
        if let Some(open_file) = self.open_files.get(&fh) {
            // Simple simulation for now
            let data = format!("GNOS Virtual File: {}\n", open_file.path.display()).into_bytes();
            
            let start = offset as usize;
            let end = std::cmp::min(start + size as usize, data.len());
            
            if start < data.len() {
                reply.data(&data[start..end]);
            } else {
                reply.data(&[]);
            }
        } else {
            reply.error(libc::EBADF);
        }
    }
    
    fn write(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        _offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyWrite,
    ) {
        debug!("write: fh={}, size={}", fh, data.len());
        
        if let Some(open_file) = self.open_files.get_mut(&fh) {
            open_file.data = Some(data.to_vec());
            info!("✍️  Wrote {} bytes to {}", data.len(), open_file.path.display());
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::EBADF);
        }
    }
    
    fn release(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: fuser::ReplyEmpty,
    ) {
        debug!("release: fh={}", fh);
        self.open_files.remove(&fh);
        reply.ok();
    }
}
