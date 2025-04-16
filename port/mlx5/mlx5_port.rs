use std::os::raw::c_void;
use rdpdk::dpdk_raw::ethdev_driver::{rte_eth_dev};
use rdpdk::dpdk_raw::rte_ethdev::{rust_get_port_eth_device};
use rdpdk::port::{DpdkPort, DpdkPortConf};
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
    dpdk_port: RawDpdkPort,

    rx_id: u32,
    tx_id: i32,
    rxq_data: *mut *mut mlx5_rxq_data,
    txq_data: *mut *mut mlx5_txq_data,
}

impl Mlx5Port {
    pub fn from(port_id: u16, port_conf: &DpdkPortConf) -> Self {

        let dpdk_port = RawDpdkPort::init(port_id, port_conf).unwrap();

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

        Mlx5Port {
            dpdk_port: dpdk_port,
            rx_id: rx_id,
            tx_id: tx_td,
            rxq_data: rxq_data,
            txq_data: txq_data,
        }
    }
}

impl DpdkPort for Mlx5Port {
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

    fn configure(&mut self) -> Result<(), String> {
        self.dpdk_port.configure()
    }
    
    fn start(&mut self) -> Result<(), String> {
        self.dpdk_port.start()
    }

    fn port_id(&self) -> u16 {
        self.dpdk_port.port_id()
    }

    fn port_conf(&self) -> &DpdkPortConf {
        self.dpdk_port.port_conf()
    }
}

use rdpdk::port::init::{
    PciVendor,
    PciDevice,
    KNOWN_PORTS,
};
use rdpdk::port::raw_port::{RawDpdkPort};

const PCI_VENDOR_ID_MLNX: PciVendor = 0x15b3;
const PCI_DEVICE_ID_MELLANOX_CONNECTX5: PciDevice = 0x1017;
const PCI_DEVICE_ID_MELLANOX_CONNECTX6: PciDevice = 0x101b;
const PCI_DEVICE_ID_MELLANOX_CONNECTX6DX: PciDevice = 0x101d;
const PCI_DEVICE_ID_MELLANOX_CONNECTX7: PciDevice = 0x1021;
const PCI_DEVICE_ID_MELLANOX_CONNECTX8: PciDevice = 0x1023;

const MLX5_PCI_DEVICES: [PciDevice;5] = [
    PCI_DEVICE_ID_MELLANOX_CONNECTX5,
    PCI_DEVICE_ID_MELLANOX_CONNECTX6,
    PCI_DEVICE_ID_MELLANOX_CONNECTX6DX,
    PCI_DEVICE_ID_MELLANOX_CONNECTX7,
    PCI_DEVICE_ID_MELLANOX_CONNECTX8,
];


fn mlx5_init_port(port_id: u16, device: PciDevice, port_conf: &DpdkPortConf) -> Result<Box<dyn DpdkPort>, String> {
    if MLX5_PCI_DEVICES.contains(&device) {
        println!("mlx5: initializing port {} for  device {:x}", port_id, device);
        return Ok(Box::new(Mlx5Port::from(port_id, port_conf)))
    }
    Err(format!("mlx5: Unsupported Mellanox device {:x}", device))
}

// Add this function to the `.init_array` section
#[unsafe(no_mangle)]
#[used]
#[unsafe(link_section = ".init_array")]
static REG_PORT_VENDOR: extern "C" fn() = register_port_ops;

extern "C" fn register_port_ops() {
    println!("Registering Mellanox port driver");
    KNOWN_PORTS.lock().unwrap().insert(PCI_VENDOR_ID_MLNX, mlx5_init_port);
}

pub fn mlx5_pol() {
    println!("mlx5_pol");
}