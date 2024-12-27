mod buffer;
mod conn;
mod utils;
mod vfs;

use buffer::LazyBuffer;
use conn::Connection;
use sqlite_vfs::register;
use std::sync::{Arc, Once, RwLock};
use utils::AtomicRuntime;
use vfs::HttpVfs;

pub use vfs::HTTP_VFS;

pub struct HttpVfsRegister {
    block_size: usize,
    download_threshold: usize,
}

impl HttpVfsRegister {
    pub fn new() -> Self {
        Self {
            block_size: 1024 * 1024 * 4,
            download_threshold: 1024,
        }
    }

    pub fn with_block_size(self, block_size: usize) -> Self {
        Self { block_size, ..self }
    }

    pub fn register(self) {
        const ONCE: Once = Once::new();

        let vfs_instance = HttpVfs {
            block_size: self.block_size,
            download_threshold: self.download_threshold,
        };

        ONCE.call_once(|| {
            let _ = register(HTTP_VFS, vfs_instance, true);
        })
    }
}

#[inline(always)]
pub fn register_http_vfs() {
    HttpVfsRegister::new().register();
}
