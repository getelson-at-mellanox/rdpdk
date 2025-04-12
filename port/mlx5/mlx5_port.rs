use std::marker::PhantomData;
use std::os::raw::c_void;
use rdpdk::dpdk_raw::ethdev_driver::{rte_eth_dev};
use rdpdk::dpdk_raw::rte_ethdev::{rust_get_port_eth_device};
use rdpdk::dpdk_raw::rte_mbuf::rte_mbuf;
use rdpdk::port::{DpdkPortData};
use crate::mlx5_raw::mlx5::mlx5_priv;
use crate::mlx5_raw::mlx5_rx::{mlx5_check_vec_rx_support, mlx5_rx_burst, mlx5_rx_burst_vec, mlx5_rxq_data};

#[path = "mlx5_raw/mlx5_raw.rs"]
pub mod mlx5_raw;

pub trait Mlx5Rx {}

pub struct Mlx5RxDef;
impl Mlx5Rx for Mlx5RxDef {}
pub struct Mlx5RxVec;
impl Mlx5Rx for Mlx5RxVec {}

unsafe impl<Rx: Mlx5Rx> Send for Mlx5Port<Rx> {}
unsafe impl<Rx: Mlx5Rx> Sync for Mlx5Port<Rx> {}
pub struct Mlx5Port<Rx: Mlx5Rx = Mlx5RxDef> {
    pub port_id: u16,
    rxq_data: *mut *mut mlx5_rxq_data,
    __phd: PhantomData<Rx>,
}

impl Mlx5Port {
    pub fn from(port_id: u16) -> Box<dyn DpdkPortData>{
        let dev: *mut rdpdk::dpdk_raw::ethdev_driver::rte_eth_dev = unsafe {
            rust_get_port_eth_device(port_id) as *mut rte_eth_dev
        };

        let mlx5_priv: &mlx5_priv =  unsafe {
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

        let rc = unsafe { mlx5_check_vec_rx_support(dev as *const _ as *mut _) };
        if rc > 0 {
            Box::new(Mlx5Port::<Mlx5RxVec> {
                port_id: port_id,
                rxq_data: rxq_data,
                __phd: Default::default()
            })
        } else {
            Box::new(Mlx5Port::<Mlx5RxDef> {
                port_id: port_id,
                rxq_data:rxq_data,
                __phd: Default::default()
            })
        }
    }
}

impl DpdkPortData for Mlx5Port<Mlx5RxDef> {
    fn rx_burst(&mut self, queue_id: u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String> {

        Ok(unsafe {
            mlx5_rx_burst(
                (*self.rxq_data).wrapping_add(queue_id as usize) as *mut c_void,
                pkts.as_ptr() as *mut _,
                pkts.len() as u16
            )
        })
    }

    fn tx_burst(&mut self, _queue_id: u16, _pkts: &[*mut rte_mbuf]) -> Result<u16, String> {
        todo!()
    }
}

impl DpdkPortData for Mlx5Port<Mlx5RxVec> {
    fn rx_burst(&mut self, queue_id: u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String> {
        Ok(unsafe {
            mlx5_rx_burst_vec(
                (*self.rxq_data).wrapping_add(queue_id as usize) as *mut c_void,
                pkts.as_ptr() as *mut _,
                pkts.len() as u16,
            )
        })
    }

    fn tx_burst(&mut self, _queue_id: u16, _pkts: &[*mut rte_mbuf]) -> Result<u16, String> {
        todo!()
    }
}

