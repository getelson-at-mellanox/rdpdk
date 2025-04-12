use std::ffi::{
    CString
};
use crate::dpdk_raw::rte_mbuf::{
    rte_pktmbuf_pool_create,
    rte_mbuf,
};

use crate::dpdk_raw::rte_ethdev::{
    rte_eth_conf,
    rte_eth_txconf,
    rte_eth_rxconf,
    rte_eth_dev_info,
    rte_mempool,
    rte_eth_dev_info_get,
};

unsafe impl Send for DpdkPortConf {}
unsafe impl Sync for DpdkPortConf {}

pub struct DpdkPortConf {
    pub dev_info: rte_eth_dev_info,
    pub dev_conf: rte_eth_conf,
    pub tx_conf: rte_eth_txconf,
    pub rx_conf: rte_eth_rxconf,
    pub rxq_num: u16,
    pub txq_num: u16,
    pub tx_desc_num: u16,
    pub rx_desc_num: u16,
}

impl DpdkPortConf {
    pub fn new_from(
        port_id: u16,
        dev_conf: rte_eth_conf,
        tx_conf: rte_eth_txconf,
        rx_conf: rte_eth_rxconf,
        rxq_num: u16,
        txq_num: u16,
        tx_desc_num: u16,
        rx_desc_num: u16,
    ) -> Result<Self, String> {
        let mut dev_info:rte_eth_dev_info = unsafe { std::mem::zeroed() };
        let _ = unsafe {
            rte_eth_dev_info_get(port_id, &mut dev_info as *mut rte_eth_dev_info)
        };

        Ok(DpdkPortConf {
            dev_info: dev_info,
            dev_conf: dev_conf,
            tx_conf: tx_conf,
            rx_conf: rx_conf,
            rxq_num: rxq_num,
            txq_num: txq_num,
            tx_desc_num: tx_desc_num,
            rx_desc_num: rx_desc_num,
        })
    }
}

pub trait DpdkPortCtrl {
    fn configure(&mut self) -> Result<(), String>;

    fn config_txq(&mut self, queue_id: u16, socket_id:u32) -> Result<rte_eth_txconf, String>;
    fn config_rxq(&mut self, queue_id: u16, pool: *mut rte_mempool, socket_id:u32) -> Result<rte_eth_rxconf, String>;

    fn start(&mut self) -> Result<(), String>;
}

pub trait DpdkPortData : Send + Sync {
    fn rx_burst(&mut self, queue_id:u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String>;
    fn tx_burst(&mut self, queue_id:u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String>;
}

pub fn alloc_mbuf_pool(
    name: &str,
    capacity: u32,
    cache_size: u32,
    priv_size: u16,
    data_root_size: u16,
    socket: i32) -> Result<*mut rte_mempool, String> {
    let pool_name = CString::new(name.to_string()).unwrap();
    let pool= unsafe {rte_pktmbuf_pool_create(
        pool_name.as_ptr(),
        capacity,
        cache_size,
        priv_size,
        data_root_size,
        socket as _
    )};
    Ok(pool as *mut _)
}

