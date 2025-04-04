use crate::cmdline::arg::{Arg, ArgData};
use num_traits::{Num, ToBytes};
use std::marker::PhantomData;
use std::str::FromStr;

pub enum ByteOrder {
    HostOrder,
    LittleEndian,
    BigEndian,
}

pub struct ArgInt<A: FromStr + Num + ToBytes> {
    bytes_order: ByteOrder,
    __x: PhantomData<A>,
}

impl<A: FromStr + Num + ToBytes> ArgInt<A> {
    pub fn new() -> Self {
        ArgInt {
            bytes_order: ByteOrder::HostOrder,
            __x: Default::default(),
        }
    }

    pub fn new_with_order(order: ByteOrder) -> Self {
        let mut arg = ArgInt::<A>::new();
        arg.bytes_order = order;
        arg
    }

    pub fn strton(&self, src: &str) -> Result<A, String> {
        match if src.starts_with("0x") || src.starts_with("0X") {
            A::from_str_radix(&src[2..src.len()], 16)
        } else if src.len() > 1 && src.starts_with('0') {
            A::from_str_radix(&src[1..src.len()], 8)
        } else {
            A::from_str_radix(src, 10)
        } {
            Ok(value) => Ok(value),
            Err(_) => Err(format!("invalid argument: \"{src}\"")),
        }
    }
}

impl<A: FromStr + Num + ToBytes> Arg for ArgInt<A> {
    fn serialize(&self, sample: &str) -> Result<ArgData, String> {
        match self.strton(sample) {
            Err(err) => return Err(err),
            Ok(val) => {
                let bytes = match self.bytes_order {
                    ByteOrder::HostOrder => val.to_ne_bytes(),
                    ByteOrder::LittleEndian => {
                        if cfg!(target_endian = "little") {
                            val.to_ne_bytes()
                        } else {
                            val.to_le_bytes()
                        }
                    },
                    ByteOrder::BigEndian => {
                        if cfg!(target_endian = "little") {
                            val.to_be_bytes()
                        } else {
                            val.to_ne_bytes()
                        }
                    }
                };

                Ok(ArgData::new_from_slice(bytes.as_ref()))},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let x16 = ArgInt::<u16>::new();
        x16.serialize("0x1234").unwrap();

        let x32 = ArgInt::<u32>::new();
        x32.serialize("0x12345678").unwrap();

        let x64 = ArgInt::<u64>::new();
        x64.serialize("0x1234567812345678").unwrap();
    }
}
