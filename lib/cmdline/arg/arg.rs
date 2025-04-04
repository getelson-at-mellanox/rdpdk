pub mod arg_int;
pub mod arg_net;

pub const ARG_DATA_SIZE: usize = 64;

pub struct ArgData {
    pub data: [u8; ARG_DATA_SIZE],
    pub size: usize,
}

impl ArgData {
    pub const DEFAULT_MASK: [u8; ARG_DATA_SIZE] = [0xff; ARG_DATA_SIZE];

    pub fn new() -> Self {
        ArgData {
            data: [0u8; ARG_DATA_SIZE],
            size: 0,
        }
    }

    pub fn new_from_size(size: usize) -> Self {
        ArgData {
            data: [0u8; ARG_DATA_SIZE],
            size: size,
        }
    }

    pub fn new_from_slice(slice: &[u8]) -> Self {
        let mut data = [0u8; ARG_DATA_SIZE];
        for i in 0..slice.len() {
            data[i] = slice[i];
        }
        ArgData {
            data: data,
            size: slice.len(),
        }
    }

    pub fn or_from_slice(&mut self, src: &[u8], offset: usize) -> Result<(), String> {
        if offset + src.len() > ARG_DATA_SIZE {
            return Err(format!(
                "Out of bounds: {ARG_DATA_SIZE} {}",
                offset + src.len()
            ));
        }
        for i in 0..src.len() {
            self.data[offset + i] |= src[i];
        }
        self.size = std::cmp::max(self.size, offset + src.len());
        Ok(())
    }

    pub fn and_from_slice(&mut self, src: &[u8], offset: usize) -> Result<(), String> {
        if offset + src.len() > ARG_DATA_SIZE {
            return Err(format!(
                "Out of bounds: {ARG_DATA_SIZE} {}",
                offset + src.len()
            ));
        }
        for i in 0..src.len() {
            self.data[offset + i] &= src[i];
        }
        self.size = std::cmp::max(self.size, offset + src.len());
        Ok(())
    }
}

pub trait Arg {
    fn serialize(&self, sample: &str) -> Result<ArgData, String>;
}

