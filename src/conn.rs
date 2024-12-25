use super::*;
use reqwest::header::{AsHeaderName, HeaderMap, ACCEPT_RANGES, CONTENT_LENGTH, RANGE};
use sqlite_vfs::{DatabaseHandle, LockKind, WalDisabled};
use std::{
    io::{Error, ErrorKind},
    sync::Mutex,
};

pub struct Connection {
    lock_state: Arc<Mutex<usize>>,
    lock: LockKind,
    rt: AtomicRuntime,
    buffer: LazyBuffer,
}

impl Connection {
    fn get_header<K>(headers: &HeaderMap, key: K) -> Option<&str>
    where
        K: AsHeaderName,
    {
        headers.get(key).and_then(|h| h.to_str().ok())
    }

    fn get_length(rt: AtomicRuntime, url: String) -> Result<usize, reqwest::Error> {
        let Some(response) = rt
            .block_on(move |client| client.get(url).send())
            .and_then(|r| r.ok())
        else {
            return Ok(0);
        };

        let headers = response.headers();
        let Some(accept_range) = Self::get_header(headers, ACCEPT_RANGES) else {
            return Ok(0);
        };

        if accept_range != "bytes" {
            return Ok(0);
        }

        let length = Self::get_header(headers, CONTENT_LENGTH)
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        Ok(length)
    }

    fn init_with_url(url: &str) -> Result<(AtomicRuntime, usize), Error> {
        let rt = AtomicRuntime::default();
        match Self::get_length(rt.clone(), url.to_string()) {
            Ok(size) => {
                if size != 0 {
                    Ok((rt, size))
                } else {
                    rt.drop();
                    Err(Error::new(
                        ErrorKind::InvalidData,
                        "database size is not a multiple of page size",
                    ))
                }
            }
            Err(e) => {
                rt.drop();
                Err(Error::new(
                    ErrorKind::Other,
                    format!("Failed to initialize db: {e}"),
                ))
            }
        }
    }

    pub fn new(url: &str) -> Result<Self, Error> {
        let (rt, size) = Self::init_with_url(url)?;
        let buffer = LazyBuffer::new(
            size,
            1024 * 1024 * 10,
            Box::new({
                let url = url.to_string();
                let rt = rt.clone();
                move |offset, size| {
                    let url = url.clone();
                    let rt = rt.clone();
                    let bytes = rt
                        .block_on(move |client| async move {
                            let response = client
                                .get(&url)
                                .header(RANGE, format!("bytes={}-{}", offset, offset + size - 1))
                                .send()
                                .await?;
                            let data = response.bytes().await?;
                            Ok::<_, reqwest::Error>(data)
                        })
                        .ok_or(Error::new(ErrorKind::Other, "runtime not initialized"))?
                        .map_err(|e| Error::new(ErrorKind::Other, format!("read error: {e}")))?;
                    Ok(bytes.to_vec())
                }
            }),
        );

        Ok(Self {
            lock_state: Arc::default(),
            lock: LockKind::None,
            rt,
            buffer,
        })
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.rt.drop();
    }
}

impl DatabaseHandle for Connection {
    type WalIndex = WalDisabled;

    fn size(&self) -> Result<u64, Error> {
        Ok(self.buffer.size() as u64)
    }

    fn read_exact_at(&mut self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        self.buffer.read(buf, offset as usize)
    }

    fn write_all_at(&mut self, _buf: &[u8], _offset: u64) -> Result<(), Error> {
        Err(Error::new(
            ErrorKind::PermissionDenied,
            "write operation is not supported",
        ))
    }

    fn sync(&mut self, _data_only: bool) -> Result<(), Error> {
        Ok(())
    }

    fn set_len(&mut self, _size: u64) -> Result<(), Error> {
        Err(Error::new(
            ErrorKind::PermissionDenied,
            "resizing the database is not supported",
        ))
    }

    fn lock(&mut self, lock: LockKind) -> Result<bool, Error> {
        let mut lock_state = self.lock_state.lock().unwrap();
        match lock {
            LockKind::None => {
                if self.lock == LockKind::Shared {
                    *lock_state -= 1;
                }
                self.lock = LockKind::None;
                Ok(true)
            }
            LockKind::Shared => {
                *lock_state += 1;
                self.lock = LockKind::Shared;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn reserved(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    fn current_lock(&self) -> Result<LockKind, Error> {
        Ok(self.lock.clone())
    }

    fn wal_index(&self, _readonly: bool) -> Result<Self::WalIndex, Error> {
        Ok(sqlite_vfs::WalDisabled::default())
    }
}