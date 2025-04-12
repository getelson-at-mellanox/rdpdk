use std::ffi::c_void;
use crate::dpdk_raw::rte_mbuf::rte_mbuf;
use crate::port::{DpdkPortConf, DpdkPortCtrl, DpdkPortData};


use crate::dpdk_raw::rte_ethdev::{
    rte_eth_conf,
    rte_eth_txconf,
    rte_eth_rxconf,
    rte_mempool,
    rte_eth_dev_start,
    rte_eth_dev_configure,
    rte_eth_tx_queue_setup,
    rte_eth_rx_queue_setup,
    rust_get_port_fp_ops,
    RTE_MAX_QUEUES_PER_PORT,
};

use crate::dpdk_raw::ethdev_driver::{
    rte_eth_fp_ops,
};

pub type RawPortRxQueue = Option<(rte_eth_rxconf, u8)>; //(rxq_conf, RTE_ETH_QUEUE_STATE_*)
pub type RawPortTxQueue = Option<(rte_eth_txconf, u8)>; //(rxq_conf, RTE_ETH_QUEUE_STATE_*)

unsafe impl Send for RawDpdkPort {}
unsafe impl Sync for RawDpdkPort {}

pub struct RawDpdkPort {
    pub port_id: u16,
    pub port_conf: DpdkPortConf,
    pub rxq: [Option<RawPortRxQueue>; RTE_MAX_QUEUES_PER_PORT as usize],
    pub txq: [Option<RawPortTxQueue>; RTE_MAX_QUEUES_PER_PORT as usize],
    raw_fp_ops: Option<*mut rte_eth_fp_ops>,
}

impl RawDpdkPort {
    pub fn init(port_id: u16, port_conf: DpdkPortConf) -> Result<Self, String> {

        let raw_fp_ops: *mut rte_eth_fp_ops = unsafe {
            rust_get_port_fp_ops(port_id) as *mut rte_eth_fp_ops
        };

        Ok(
            RawDpdkPort {
                port_id: port_id,
                port_conf: port_conf,
                rxq: [None; RTE_MAX_QUEUES_PER_PORT as usize],
                txq: [None; RTE_MAX_QUEUES_PER_PORT as usize],
                raw_fp_ops: Some(raw_fp_ops),
            }
        )
    }
}

impl DpdkPortCtrl for RawDpdkPort {
    fn configure(&mut self) -> Result<(), String> {
        let _ = unsafe {rte_eth_dev_configure(
            self.port_id,
            self.port_conf.rxq_num,
            self.port_conf.txq_num,
            &self.port_conf.dev_conf as *const rte_eth_conf)};
        Ok(())
    }

    fn start(&mut self) -> Result<(), String> {
        unsafe {rte_eth_dev_start(self.port_id)};
        Ok(())
    }

    fn config_txq(&mut self, queue_id:u16, socket_id: u32) -> Result<rte_eth_txconf, String> {
        let tx_conf: rte_eth_txconf = unsafe { std::mem::zeroed() };

        let _ = unsafe {rte_eth_tx_queue_setup(
            self.port_id,
            queue_id,
            self.port_conf.tx_desc_num,
            socket_id,
            &tx_conf as *const _ as *mut _
        )};

        Ok(tx_conf)
    }

    fn config_rxq(&mut self, queue_id:u16, pool: *mut rte_mempool, socket_id:u32) -> Result<rte_eth_rxconf, String> {
        let mut rxq_conf: rte_eth_rxconf = self.port_conf.dev_info.default_rxconf.clone();
        rxq_conf.offloads = 0;

        let _ = unsafe{rte_eth_rx_queue_setup(
            self.port_id,
            queue_id,
            self.port_conf.rx_desc_num,
            socket_id,
            &rxq_conf as *const _ as *mut _,
            pool
        )};

        Ok(rxq_conf)
    }

}

impl DpdkPortData for RawDpdkPort {
    fn rx_burst(&mut self, queue_id:u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String> {

        let ops = unsafe { &mut *self.raw_fp_ops.unwrap() };
        let rxqd:*mut c_void = unsafe { *ops.rxq.data.wrapping_add(queue_id as usize) };

        let nb_rx: u16 = unsafe {
            let rxfn = ops.rx_pkt_burst.unwrap();
            rxfn(rxqd, pkts.as_ptr() as _, pkts.len() as u16)
        };

        Ok(nb_rx)
    }

    fn tx_burst(&mut self, queue_id:u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String> {
        let ops = unsafe { &mut *self.raw_fp_ops.unwrap() };
        let txqd:*mut c_void = unsafe { *ops.txq.data.wrapping_add(queue_id as usize) };
        let nb_tx: u16 = unsafe {
            let txfn = ops.tx_pkt_burst.unwrap();
            txfn(txqd, pkts.as_ptr() as _, pkts.len() as u16)
        };

        Ok(nb_tx)
    }
}
