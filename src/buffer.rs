use super::*;
use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Result},
};

pub struct LazyBlock {
    data: Vec<u8>,
    size: usize,
}

impl LazyBlock {
    fn eof() -> Error {
        Error::new(ErrorKind::UnexpectedEof, "read out of bounds")
    }

    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize> {
        let size = buf.len();
        if size == 0 {
            return Ok(0);
        }
        if offset >= self.size {
            return Err(Self::eof());
        }
        let end = offset + size;
        if end > self.size {
            return Err(Self::eof());
        }
        buf.copy_from_slice(&self.data[offset..end]);
        Ok(end - offset)
    }
}

type Fetch = Box<dyn Fn(usize, usize) -> Result<Vec<u8>> + Send + Sync>;

pub struct LazyBuffer {
    blocks: HashMap<usize, LazyBlock>,
    block_size: usize,
    download_threshold: usize,
    length: usize,
    fetch: Arc<Fetch>,
}

impl LazyBuffer {
    pub fn new(length: usize, block_size: usize, download_threshold: usize, fetch: Fetch) -> Self {
        Self {
            blocks: HashMap::new(),
            block_size,
            download_threshold,
            length,
            fetch: Arc::new(fetch),
        }
    }

    pub fn size(&self) -> usize {
        self.length
    }

    fn get_block(&mut self, block_index: usize) -> Result<&LazyBlock> {
        if !self.blocks.contains_key(&block_index) {
            let offset = block_index * self.block_size;
            let size = self.block_size.min(self.length - offset);
            let data = (self.fetch)(offset, size)?;
            self.blocks.insert(block_index, LazyBlock { data, size });
        }
        self.blocks.get(&block_index).ok_or(Error::new(
            ErrorKind::UnexpectedEof,
            "block not found after fetch",
        ))
    }

    pub fn read(&mut self, buf: &mut [u8], offset: usize) -> Result<()> {
        let end = offset + buf.len();
        if end > self.length {
            return Err(Error::new(ErrorKind::UnexpectedEof, "read out of bounds"));
        }
        let block_index = offset / self.block_size;
        let block_offset = offset - block_index * self.block_size;
        if !self.blocks.contains_key(&block_index) && buf.len() <= self.download_threshold {
            // skip full block fetch if buf is smaller than download_threshold
            let data = (self.fetch)(offset, buf.len())?;
            if data.len() != buf.len() {
                return Err(Error::new(ErrorKind::UnexpectedEof, "read out of bounds"));
            }
            buf.copy_from_slice(&data);
        } else {
            self.get_block(block_index)?.read(buf, block_offset)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_fetch(_offset: usize, size: usize) -> Result<Vec<u8>> {
        Ok(vec![1; size])
    }

    #[test]
    fn test_lazy_buffer() {
        let block_size = 16;
        let total_size = 64;

        let mut lazy_buffer = LazyBuffer::new(total_size, block_size, 0, Box::new(mock_fetch));

        let mut buf1 = vec![0u8; 16];
        lazy_buffer.read(&mut buf1, 0).unwrap();
        assert_eq!(buf1, vec![1; 16]);

        let mut buf2 = vec![0u8; 16];
        lazy_buffer.read(&mut buf2, 0).unwrap();
        assert_eq!(buf2, vec![1; 16]);

        let mut buf3 = vec![0u8; 16];
        lazy_buffer.read(&mut buf3, 16).unwrap();
        assert_eq!(buf3, vec![1; 16]);

        // should not read across block
        let mut buf4 = vec![0u8; 32];
        let result = lazy_buffer.read(&mut buf4, 32);
        assert!(result.is_err());

        // should not read across block
        let mut buf5 = vec![0u8; 16];
        let result = lazy_buffer.read(&mut buf5, 24);
        assert!(result.is_err());

        // should not read out of bounds
        let mut buf = vec![0u8; 16];
        let result = lazy_buffer.read(&mut buf, 70);
        assert!(result.is_err());
    }
}
