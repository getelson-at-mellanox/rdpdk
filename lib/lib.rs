use std::ffi::{CString, CStr};
use crate::lib::dpdk_raw::rte_ethdev::{rte_eth_dev_socket_id, rte_lcore_to_socket_id};
use crate::lib::dpdk_raw::rte_mbuf::rte_pktmbuf_pool_create;
use crate::lib::dpdk_raw::rte_mempool::{rte_mempool};

#[path = "dpdk_raw/dpdk_raw.rs"]
pub mod dpdk_raw;

#[derive(Clone)]
pub struct MBuffMempool {
    pool_ptr: *mut rte_mempool
}

pub struct MBuffMempoolHandle {
    name: String,
    capacity: u32,
    priv_size: u16,
    data_root_size: u16,
    cache_size: u32,
    socket: i32,
}

impl MBuffMempoolHandle {
    pub fn new(name: &str, capacity: u32) -> Self {
        MBuffMempoolHandle {
            name: String::from(name),
            capacity: capacity,
            priv_size: 0,
            data_root_size: 0,
            cache_size: 0,
            socket: NumaSocketId::NumaSocketIdAny.to_socket_id(),
        }
    }
    
    pub fn mempool_create(&mut self) -> Result<MBuffMempool, String>{
        let mut pool_ptr = unsafe {
            rte_pktmbuf_pool_create(
                CString::new(self.name.as_str()).unwrap().as_ptr(),
                self.capacity,
                self.priv_size as _,
                self.cache_size as _,
                self.data_root_size as _,
                self.socket as _
            )};

        if pool_ptr != std::ptr::null_mut() {
            Ok((MBuffMempool {pool_ptr: pool_ptr as _}))
        } else {
            Err(String::from("Failed to create mempool"))
        }
    }
    
    pub fn cache_size(&mut self, cs:u32) -> &mut Self {
        self.cache_size = cs;
        return self;
    }
    pub fn priv_size(&mut self, ps:u16) -> &mut Self {
        self.priv_size = ps;
        return self;
    }
    pub fn data_root_size(&mut self, drs:u16) -> &mut Self {
        self.data_root_size = drs;
        return self;
    }
    pub fn socket(&mut self, s: i32) -> &mut Self {
        self.socket = s;
        return self;
    }
}

impl std::fmt::Debug for MBuffMempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mpool = unsafe { &*(self.pool_ptr) };
        unsafe {
            write!(f,
                "Mempool {} address: {:p} ",
                CStr::from_ptr(mpool.name.as_ptr()).to_str().unwrap(),
                self.pool_ptr)
        }
    }
}


const SOCKET_ID_ANY: i32 = -1;
pub enum NumaSocketId {
    NumaSocketIdAny,
    NumaSocketIdPort(u16),
    NumaSocketIdLCore(u32),
}

impl NumaSocketId {
    

    pub fn to_socket_id(&self) -> i32 {
        match self {
            NumaSocketId::NumaSocketIdAny => SOCKET_ID_ANY,
            NumaSocketId::NumaSocketIdPort(port_id) => {
                unsafe { rte_eth_dev_socket_id(*port_id) as i32 } 
            },
            NumaSocketId::NumaSocketIdLCore(lcore) => {
                unsafe { rte_lcore_to_socket_id(*lcore as _) as i32 }
            },
        }
    }
}