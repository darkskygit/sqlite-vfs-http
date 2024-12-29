mod buffer;
mod conn;
mod utils;
mod vfs;

use buffer::LazyBuffer;
use conn::Connection;
use reqwest::Client;
use sqlite_vfs::{register, RegisterError};
use std::sync::{Arc, Once, RwLock};
use utils::AtomicRuntime;
use vfs::HttpVfs;

const SQLITE_PAGE_SIZE: usize = 1024 * 4;
pub const HTTP_VFS: &str = "http";

pub struct HttpVfsRegister {
    /// how many pages in block, default is 8MB, 2048 pages
    block_size: usize,
    /// default client
    client: Option<Client>,
    /// read the first few pages of each block without downloading the entire block
    download_threshold: usize,
    /// sqlite's page size is 4KB by default
    page_size: usize,
}

impl HttpVfsRegister {
    pub fn new() -> Self {
        Self {
            client: None,
            block_size: SQLITE_PAGE_SIZE * 1024 * 2,
            download_threshold: 0,
            page_size: SQLITE_PAGE_SIZE,
        }
    }

    pub fn with_block_size(self, page_num: usize) -> Self {
        Self {
            block_size: page_num * self.page_size,
            ..self
        }
    }

    pub fn with_client(self, client: Client) -> Self {
        Self {
            client: Some(client),
            ..self
        }
    }

    /// Set how many page read don't download full block
    pub fn with_download_threshold(self, page_num: usize) -> Self {
        Self {
            download_threshold: page_num * self.page_size,
            ..self
        }
    }

    pub fn with_page_size(self, page_size: usize) -> Self {
        Self { page_size, ..self }
    }

    pub fn register(self) -> Result<(), RegisterError> {
        let vfs_instance = HttpVfs {
            client: self.client,
            block_size: self.block_size,
            download_threshold: self.download_threshold,
        };
        register(HTTP_VFS, vfs_instance, true)
    }
}

/// register http vfs, use `Once` internally to ensure only register once
#[inline(always)]
pub fn register_http_vfs() {
    const ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let _ = HttpVfsRegister::new().register();
    })
}

/// register http vfs with custom client
/// use `Once` internally to ensure only register once
#[inline(always)]
pub fn register_http_vfs_with_custom(cb: impl FnOnce(HttpVfsRegister) -> HttpVfsRegister) {
    const ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let _ = cb(HttpVfsRegister::new()).register();
    })
}
