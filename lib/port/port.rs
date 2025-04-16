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

/// Configuration details for a DPDK port.
///
/// # Overview
///
/// `DpdkPortConf` is used to initialize and configure ports in a DPDK environment. It includes:
/// - Device information (`dev_info`) pulled from DPDK.
/// - Transmit and receive configurations, including queue settings and memory pools.
/// - Details about the number of queues and descriptors for transmission and reception.
///
/// This struct is typically instantiated using the [`DpdkPortConf::new_from`] method.
///
/// # Example
///
/// ```
/// let conf = DpdkPortConf::new_from(
///     port_id,
///     dev_conf,
///     tx_conf,
///     rx_conf,
///     8,               // Number of RX queues
///     8,               // Number of TX queues
///     512,             // RX descriptors
///     512,             // TX descriptors
///     0,               // RX queue socket ID
///     0,               // TX queue socket ID
///     Some(rx_mempool) // RX queue mempool
/// ).unwrap();
/// ```
/// 
#[derive(Clone)]
pub struct DpdkPortConf {
    /// Information about the DPDK Ethernet device (e.g., driver, capabilities, etc.).
    pub dev_info: rte_eth_dev_info,

    /// Configuration for the Ethernet device. Determines the overall behavior of the device.
    pub dev_conf: rte_eth_conf,

    /// Configuration for transmitting (Tx) packets on the Ethernet device.
    pub tx_conf: rte_eth_txconf,

    /// Configuration for receiving (Rx) packets on the Ethernet device.
    pub rx_conf: rte_eth_rxconf,

    /// Number of receive (Rx) queues configured on this port.
    pub rxq_num: u16,

    /// Number of transmit (Tx) queues configured on this port.
    pub txq_num: u16,

    /// Number of descriptors for each transmit (Tx) queue.
    /// Descriptors represent items in a queue to handle packets.
    pub tx_desc_num: u16,

    /// Number of descriptors for each receive (Rx) queue.
    pub rx_desc_num: u16,

    /// NUMA socket ID associated with the memory used by receive (Rx) queues.
    pub rxq_socket_id: u32,

    /// NUMA socket ID associated with the memory used by transmit (Tx) queues.
    pub txq_socket_id: u32,

    /// Memory pool associated with receive (Rx) queues.
    /// This manages the buffers used for storing incoming packets.
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

unsafe impl Send for DpdkPortConf {}
unsafe impl Sync for DpdkPortConf {}

/// A trait defining basic operations for managing and interacting with a DPDK port.
///
/// # Overview
///
/// The `DpdkPort` trait standardizes how to operate on DPDK ports, making it possible to:
/// - Configure the Ethernet device using [`configure`].
/// - Start the device using [`start`].
/// - Handle Rx and Tx bursts of packets using [`rx_burst`] and [`tx_burst`].
///
pub trait DpdkPort: Send + Sync {
    /// Returns the port ID of the DPDK port.
    ///
    /// # Return Value
    /// A `u16` that uniquely identifies the DPDK port.
    ///
    /// # Example
    /// ```
    /// let port_id = dpdk_port.port_id();
    /// println!("DPDK port ID: {}", port_id);
    /// ```
    fn port_id(&self) -> u16;

    /// Returns a reference to the configuration object of the DPDK port.
    ///
    /// # Return Value
    /// A reference to [`DpdkPortConf`], which contains various settings like Rx/Tx queue configurations,
    /// memory pools, and NUMA socket IDs.
    ///
    /// # Example
    /// ```
    /// let port_config = dpdk_port.port_conf();
    /// println!("Rx queues: {}", port_config.rxq_num);
    /// ```
    fn port_conf(&self) -> &DpdkPortConf;

    /// Configures the DPDK Ethernet device with the settings specified in the port configuration.
    ///
    /// This method is typically called before starting the port to ensure it is prepared for Rx and Tx operations.
    ///
    /// # Return Value
    /// - `Ok(())` if the configuration was applied successfully.
    /// - `Err(String)` with a descriptive error message if the configuration failed.
    ///
    /// # Example
    /// ```
    /// let result = dpdk_port.configure();
    /// if let Err(err) = result {
    ///     eprintln!("Failed to configure the port: {}", err);
    /// }
    /// ```
    fn configure(&mut self) -> Result<(), String>;

    /// Starts the DPDK Ethernet device.
    ///
    /// This method initializes the Rx and Tx queues, making the port ready for data transmission
    /// and reception.
    ///
    /// # Return Value
    /// - `Ok(())` if the port was started successfully.
    /// - `Err(String)` if the startup process failed, with a descriptive error message.
    ///
    /// # Example
    /// ```
    /// let result = dpdk_port.start();
    /// if let Err(err) = result {
    ///     eprintln!("Failed to start the port: {}", err);
    /// }
    /// ```
    fn start(&mut self) -> Result<(), String>;

    /// Receives a burst of packets on the specified Rx queue.
    ///
    /// # Parameters
    /// - `queue_id`: The ID of the Rx queue to receive packets from.
    /// - `pkts`: A mutable reference to an array of packet buffers (`*mut rte_mbuf`) where received packets
    ///   will be written.
    ///
    /// # Return Value
    /// - `Ok(u16)` containing the number of packets successfully received.
    /// - `Err(String)` if the operation failed.
    ///
    /// # Example
    /// ```
    /// let pkts: Vec<*mut rte_mbuf> = vec![std::ptr::null_mut(); 32];
    /// let received = dpdk_port.rx_burst(0, &pkts);
    /// match received {
    ///     Ok(count) => println!("Received {} packets", count),
    ///     Err(err) => eprintln!("Rx burst failed: {}", err),
    /// }
    /// ```
    fn rx_burst(&mut self, queue_id: u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String>;

    /// Sends a burst of packets on the specified Tx queue.
    ///
    /// # Parameters
    /// - `queue_id`: The ID of the Tx queue to send packets on.
    /// - `pkts`: A reference to an array of packet buffers (`*mut rte_mbuf`) to send.
    ///
    /// # Return Value
    /// - `Ok(u16)` containing the number of packets successfully sent.
    /// - `Err(String)` if the operation failed.
    ///
    /// # Example
    /// ```
    /// let pkts: Vec<*mut rte_mbuf> = vec![some_packet_ptr1, some_packet_ptr2];
    /// let sent = dpdk_port.tx_burst(0, &pkts);
    /// match sent {
    ///     Ok(count) => println!("Sent {} packets", count),
    ///     Err(err) => eprintln!("Tx burst failed: {}", err),
    /// }
    /// ```
    fn tx_burst(&mut self, queue_id: u16, pkts: &[*mut rte_mbuf]) -> Result<u16, String>;
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
