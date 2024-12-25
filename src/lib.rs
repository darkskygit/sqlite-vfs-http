mod buffer;
mod conn;
mod http;
mod utils;

use buffer::LazyBuffer;
use conn::Connection;
use http::HttpVfs;
use sqlite_vfs::register;
use std::sync::{Arc, Once, RwLock};
use utils::AtomicRuntime;

pub use http::HTTP_VFS;

pub fn register_http_vfs() {
    const ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let _ = register(HTTP_VFS, HttpVfs::default(), true);
    })
}
