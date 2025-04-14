use std::os::raw::c_void;
use rdpdk::dpdk_raw::ethdev_driver::{rte_eth_dev};
use rdpdk::dpdk_raw::rte_ethdev::{rust_get_port_eth_device};
use rdpdk::port::{DpdkPortData};
use crate::mlx5_raw::mlx5::{
    mlx5_priv,
    mlx5_select_rx_function_index,
    mlx5_select_tx_function_index
};
use crate::mlx5_raw::mlx5_rx::{mlx5_rx_functions, mlx5_rxq_data};
use crate::mlx5_raw::mlx5_tx::{mlx5_txq_data, txoff_func};
use rdpdk::dpdk_raw::rte_mbuf::rte_mbuf;

#[path = "mlx5_raw/mlx5_raw.rs"]
pub mod mlx5_raw;

unsafe impl Send for Mlx5Port {}
unsafe impl Sync for Mlx5Port {}

pub struct Mlx5Port {
    pub port_id: u16,
    rx_id: u32,
    tx_id: i32,
    rxq_data: *mut *mut mlx5_rxq_data,
    txq_data: *mut *mut mlx5_txq_data,
}

impl Mlx5Port {
    pub fn from(port_id: u16) -> Box<dyn DpdkPortData>{
        let dev: *mut rdpdk::dpdk_raw::ethdev_driver::rte_eth_dev = unsafe {
            rust_get_port_eth_device(port_id) as *mut rte_eth_dev
        };

        let mlx5_priv: &mlx5_priv = unsafe {
            let data = (&mut *dev).data.as_mut().unwrap();
            (data.dev_private as *mut mlx5_priv).as_mut().unwrap()
        };

        let rxq_data = unsafe {
            mlx5_priv.
                dev_data.
                as_ref()
                .unwrap()
                .rx_queues as *mut *mut mlx5_rxq_data
        };

        // TODO: fix eth_dev initialization
        let rx_id = 0 * unsafe { mlx5_select_rx_function_index(dev as *mut _) };
        let tx_td = unsafe { mlx5_select_tx_function_index(dev as *mut _)};

        let txq_data = unsafe {
            mlx5_priv.
                dev_data.
                as_ref()
                .unwrap()
                .tx_queues as *mut *mut mlx5_txq_data
        };

        Box::new(Mlx5Port {
            port_id: port_id,
            rx_id: rx_id,
            tx_id: tx_td,
            rxq_data: rxq_data,
            txq_data: txq_data,
        })
    }
}

impl DpdkPortData for Mlx5Port {
    fn rx_burst(&mut self, queue_id: u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String> {

        let rxfn = unsafe {
            mlx5_rx_functions
                .as_ptr()
                .wrapping_add(self.rx_id as usize)
                .as_ref()
                .unwrap()
                .unwrap()
        };

        Ok(unsafe {
            rxfn(
                (*self.rxq_data).wrapping_add(queue_id as usize) as *mut c_void,
                pkts.as_ptr() as *mut _,
                pkts.len() as u16
            )
        })
    }

    fn tx_burst(&mut self, queue_id: u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String> {

        let txfn = unsafe {
            txoff_func
                .as_ptr()
                .wrapping_add(self.tx_id as usize)
                .as_ref()
                .unwrap()
                .func
                .unwrap()
        };

        Ok(unsafe {
            txfn(
                (*self.txq_data).wrapping_add(queue_id as usize) as *mut c_void,
                pkts.as_ptr() as *mut _,
                pkts.len() as u16,
            )
        })
    }
}