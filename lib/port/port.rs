pub mod raw_port;
pub mod init;

use std::ffi::{
    CString
};
use std::sync::Arc;
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

#[derive(Clone)]
pub struct DpdkPortConf {
    pub dev_info: rte_eth_dev_info,
    pub dev_conf: rte_eth_conf,
    pub tx_conf: rte_eth_txconf,
    pub rx_conf: rte_eth_rxconf,
    pub rxq_num: u16,
    pub txq_num: u16,
    pub tx_desc_num: u16,
    pub rx_desc_num: u16,
    pub rxq_socket_id: u32,
    pub txq_socket_id: u32,
    pub rxq_mempool: Option<Arc<DpdkMempool>>,
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
        rxq_socket_id: u32,
        txq_socket_id: u32,
        rxq_mempool: Option<Arc<DpdkMempool>>,
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
            rxq_socket_id: rxq_socket_id,
            txq_socket_id: txq_socket_id,
            rxq_mempool: rxq_mempool,
        })
    }
}

pub trait DpdkPort: Send + Sync {
    fn port_id(&self) -> u16;
    fn port_conf(&self) -> &DpdkPortConf;

    fn configure(&mut self) -> Result<(), String>;

    fn start(&mut self) -> Result<(), String>;

    fn rx_burst(&mut self, queue_id:u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String>;
    fn tx_burst(&mut self, queue_id:u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String>;
}

pub trait DpdkFlow : DpdkPort {}

pub trait DpdkTmplFlow : DpdkPort {}

pub struct DpdkMempool {
    pub pool: *mut rte_mempool,
}

pub fn alloc_mbuf_pool(
    name: &str,
    capacity: u32,
    cache_size: u32,
    priv_size: u16,
    data_root_size: u16,
    socket: i32) -> Result<DpdkMempool, String> {
    let pool_name = CString::new(name.to_string()).unwrap();
    let pool= unsafe {rte_pktmbuf_pool_create(
        pool_name.as_ptr(),
        capacity,
        cache_size,
        priv_size,
        data_root_size,
        socket as _
    )};
    Ok(DpdkMempool {pool: pool as *mut _})
}
