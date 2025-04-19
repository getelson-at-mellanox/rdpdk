use std::ffi::{c_char, CString};
use crate::lib::port::*;
use crate::lib::dpdk_raw::rte_eal::{rte_eal_cleanup, rte_eal_init};
use crate::lib::dpdk_raw::rte_ethdev::rte_eth_dev_count_avail;

#[derive(Debug)]
pub struct Eal {
    eth_ports: Option<Vec<Port>>,
}

impl Eal {
    //  allow init once,
    pub fn init(eal_args:&String) -> Result<Self, String> {
        
        let mut argv: Vec<*mut c_char> = eal_args
            .split_ascii_whitespace()
            .collect::<Vec<&str>>()
            .iter()
            .map(|arg| CString::new(arg.as_bytes()).unwrap().into_raw())
            .collect();
        
        let rc = unsafe { rte_eal_init(argv.len() as i32, argv.as_mut_ptr()) };
        if rc < 0 {
            unsafe {rte_eal_cleanup()};
            return Err(format!("rte_eal_init failed with rc {}", rc));
        }
        
        let ports_num = unsafe { rte_eth_dev_count_avail() };
        
        let mut eth_ports = Vec::with_capacity(ports_num as usize);
        for port_id in 0..ports_num {
            eth_ports.push(Port::from_port_id(port_id))
        }
        
        Ok(Eal {
            eth_ports: Some(eth_ports),
        })
    }

    // API to get eth ports, taking ownership. It can be called once.
    // The return will be None for future calls
    pub fn take_eth_ports(&mut self) -> Option<Vec<Port>> {
        self.eth_ports.take()
    }
}

impl Drop for Eal {
    fn drop(&mut self) {
        unsafe { rte_eal_cleanup() };
    }
}
