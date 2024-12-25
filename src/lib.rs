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

pub fn register_http_vfs() {
    const ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let _ = register(HTTP_VFS, HttpVfs::default(), true);
    })
}
