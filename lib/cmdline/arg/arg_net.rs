use crate::cmdline::arg::{Arg, ArgData};

pub struct EthAddrArg;

impl EthAddrArg {
    pub fn new() -> EthAddrArg {
        EthAddrArg {}
    }
}

impl Arg for EthAddrArg {
    fn serialize(&self, sample: &str) -> Result<ArgData, String> {
        let mut arg = ArgData::new_from_size(6);

        for (i, val) in sample.split(':').enumerate() {
            arg.data[i] = u8::from_str_radix(val, 16).unwrap()
        }
        Ok(arg)
    }
}

pub struct Ipv4AddrArg;
impl Ipv4AddrArg {
    pub fn new() -> Ipv4AddrArg { Ipv4AddrArg {} }
}

impl Arg for Ipv4AddrArg {
    fn serialize(&self, sample: &str) -> Result<ArgData, String> {
        let mut arg = ArgData::new_from_size(4);
        for (i, val) in sample.split('.').enumerate() {
            arg.data[i] = match val.parse::<u8>() {
                Ok(v) => v,
                Err(_) => return Err(format!("invalid argument: \"{sample}\"")),
            }
        }
        Ok(arg)
    }
}
