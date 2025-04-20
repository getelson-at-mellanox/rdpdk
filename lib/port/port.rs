use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::Arc;
use crate::lib::{DpdkMempool, MBuffMempoolHandle, NumaSocketId};
use crate::lib::dpdk_raw::rte_ethdev::{
    rte_eth_rss_conf,
    rte_eth_conf,
    rte_eth_hash_function,
    rte_eth_rxconf,
    rte_eth_txconf,
    rte_eth_dev_configure,
    rte_eth_rx_queue_setup,
    rte_eth_tx_queue_setup,
    rte_eth_hash_function_RTE_ETH_HASH_FUNCTION_DEFAULT,
    rust_get_port_fp_ops,
    rte_eth_dev_start,
    RTE_ETH_RSS_IP,
    rte_eth_dev_info,
    rte_eth_dev_info_get,
    eth_rx_burst_t,
    eth_tx_burst_t,
};
use crate::lib::dpdk_raw::rte_mbuf::rte_mbuf;

#[derive(Clone)]
pub(crate) struct DpdkEthRxConf {
    rx_conf: rte_eth_rxconf,
}

unsafe impl Send for DpdkEthRxConf {}
unsafe impl Sync for DpdkEthRxConf {}

impl DpdkEthRxConf {
    pub(crate) fn new() -> Self {
        DpdkEthRxConf {
            rx_conf: unsafe { std::mem::zeroed() },
        }
    }
}

impl std::fmt::Debug for DpdkEthRxConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "eth_rx_conf: {{ todo }}")
    }
}

#[derive(Clone)]
pub(crate) struct DpdkEthTxConf {
    tx_conf: rte_eth_txconf,
}

unsafe impl Send for DpdkEthTxConf {}
unsafe impl Sync for DpdkEthTxConf {}

impl DpdkEthTxConf {
    pub(crate) fn new() -> Self {
        DpdkEthTxConf {
            tx_conf: unsafe { std::mem::zeroed() },
        }
    }
}

impl std::fmt::Debug for DpdkEthTxConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "eth_tx_conf: {{ todo }}")
    }
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub(crate) struct PortHandle {
    rxq_num: Option<u16>,
    txq_num: Option<u16>,
    rss_hash_key: Option<String>,
    rss_hf: u64, // see rte_ethdev::RTE_ETH_RSS_*
    rss_hfa: rte_eth_hash_function,
    _rx_conf: DpdkEthRxConf,
    _tx_conf: DpdkEthTxConf,
    refcnt: Arc<()>
}

impl std::fmt::Debug for rte_eth_conf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dev_conf: {{ todo }}")
    }
}

impl std::fmt::Debug for rte_eth_rxconf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dev_rxconf: {{ todo }}")
    }
}

impl std::fmt::Debug for rte_eth_txconf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dev_txconf: {{ todo }}")
    }
}

impl std::fmt::Debug for rte_eth_dev_info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dev_info: {{ todo }}")
    }
}

impl PortHandle {
    fn new() -> Self {
        PortHandle {
            rxq_num: None,
            txq_num: None,
            rss_hash_key: None,
            rss_hf: 0,
            rss_hfa: rte_eth_hash_function_RTE_ETH_HASH_FUNCTION_DEFAULT,
            _rx_conf: DpdkEthRxConf::new(),
            _tx_conf: DpdkEthTxConf::new(),
            refcnt: Arc::new(())
        }
    }

    fn stop(&mut self) -> Result<(), usize> {
        let sc = Arc::<()>::strong_count(&self.refcnt);
        if  sc == 1 {
            Ok(())
        } else {
            Err(sc)
        }
    }
}

#[derive(Debug)]
pub struct TxqHandle {
    _handle: PortHandle,
    port_id: u16,
    queue_id: u16,
    desc_num: u16,
}

impl TxqHandle {
    pub fn queue_setup(self) -> Result<Txq, String> {
        let tx_conf: rte_eth_txconf = unsafe { std::mem::zeroed() };

        let rc = unsafe {
            rte_eth_tx_queue_setup(
                self.port_id,
                self.queue_id,
                self.desc_num,
                NumaSocketId::NumaSocketIdPort(self.port_id).to_socket_id() as _,
                &tx_conf as *const _,
            )
        };
        if rc < 0 {
            return Err(format!("port-{}: rte_eth_rx_queue_setup failed: {}", self.port_id, rc));
        }

        Ok(Txq {
            handle: self,
            _tx_conf:tx_conf,
            tx_burst: None,
            _phantom: std::marker::PhantomData,
        })
    }
}

// Handle allows moving between threads, its not polling!
#[derive(Debug)]
pub struct RxqHandle {
    _handle: PortHandle,
    port_id: u16,
    queue_id: u16,
    desc_num: u16,
    mempool: DpdkMempool,
}

impl RxqHandle {
    pub(crate) fn new(
        handle: PortHandle,
        port_id: u16,
        queue_id: u16,
        desc_num: u16,
        mempool: DpdkMempool) -> Self {
        RxqHandle {
            _handle: handle,
            port_id: port_id,
            queue_id: queue_id,
            desc_num: desc_num,
            mempool: mempool
        }
    }

    // This function is the key to the API design: it ensures the rx_burst()
    // function is only available via the Rxq struct, after enable_polling() has been called.
    // It "consumes" (takes "self" as a parameter, not a '&' reference!) which essentially
    // destroys/invalidates the handle from the Application level code.

    // It returns an Rxq instance, which has the PhantomData to encode the threading requirements,
    // and the Rxq has the rx_burst() function: this allows the application to recieve packets.
    pub fn queue_setup(mut self) -> Result<Rxq, String> {
        let rx_conf: rte_eth_rxconf = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            rte_eth_rx_queue_setup(
                self.port_id,
                self.queue_id,
                self.desc_num,
                NumaSocketId::NumaSocketIdPort(self.port_id).to_socket_id() as _,
                &rx_conf as *const _,
                self.mempool.pool_ptr as *mut _
            )
        };

        if rc < 0 {
            return Err(format!("port-{}: rte_eth_rx_queue_setup failed: {}", self.port_id, rc));
        }

        Ok(Rxq {
            handle: self,
            _rx_conf:rx_conf,
            rx_burst: None,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[derive(Debug)]
pub struct Rxq {
    handle: RxqHandle,
    _rx_conf: rte_eth_rxconf,
    rx_burst:Option<(eth_rx_burst_t, *mut c_void)>,
    _phantom: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Rxq {

    // Called after port start.
    // Requires 0001-rust-export-missing-port-objects.patch.
    pub fn enable_rx(&mut self) {
        self.rx_burst = unsafe {
            let ops = &mut *rust_get_port_fp_ops(self.handle.port_id);
            let rxqd:*mut c_void = *ops.rxq.data.wrapping_add(self.handle.queue_id as usize);
            Some((ops.rx_pkt_burst, rxqd))
        }
    }

    pub fn rx_burst(&mut self, mbufs: &[*mut rte_mbuf]) -> u16 {

        let num = unsafe {
            let (burst_fn, qdesc) = self.rx_burst.unwrap();
            burst_fn.unwrap()(qdesc, mbufs.as_ptr() as _, mbufs.len() as u16)
        };
        num
    }
}

pub struct Txq {
    handle: TxqHandle,
    _tx_conf: rte_eth_txconf,
    tx_burst: Option<(eth_tx_burst_t, *mut c_void)>,
    _phantom: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Txq {
    // Requires 0001-rust-export-missing-port-objects.patch
    pub fn enable_tx(&mut self) {
        self.tx_burst = unsafe {
            let ops = &mut *rust_get_port_fp_ops(self.handle.port_id);
            let txqd:*mut c_void = *ops.txq.data.wrapping_add(self.handle.queue_id as usize);
            Some((ops.tx_pkt_burst, txqd))
        }
    }

    pub fn tx_burst(&mut self, mbufs: &[*mut rte_mbuf]) -> u16 {
        let num = unsafe {
            let (burst_fn, qdesc) = self.tx_burst.unwrap();
            burst_fn.unwrap()(qdesc, mbufs.as_ptr() as _, mbufs.len() as u16)
        };
        num
    }
}

#[derive(Debug)]
pub struct Port {
    port_id: u16,
    handle: PortHandle,
    
    rxqs: Option<Vec<RxqHandle>>,
    txqs: Option<Vec<TxqHandle>>,

    dev_info: rte_eth_dev_info,
    dev_conf: rte_eth_conf,
}

pub type PortId = u16;
impl Port {
    pub fn id(&self) -> PortId {
        self.port_id
    }

    pub fn from_port_id(port_id: PortId) -> Self {
        let mut dev_info: rte_eth_dev_info = unsafe { std::mem::zeroed() };
        let _ = unsafe {
            rte_eth_dev_info_get(port_id, &mut dev_info as *mut rte_eth_dev_info)
        };

        Port {
            handle: PortHandle::new(),
            port_id:port_id,
            rxqs: None,
            txqs: None,

            dev_info: dev_info,
            dev_conf: unsafe { std::mem::zeroed() },
        }
    }

    // Port handle is cloned to each Rxq and Txq because the handle is not moved between threads.
    // Therefore, rte_eth_dev_configure() must be called before queues are configured.
    pub fn configure(&mut self, rxq_num: u16, txq_num: u16) -> Result<(), String> {
        if self.rxqs.is_some() || self.txqs.is_some() {
            return Err(format!("port-{}: Rx/Tx queues already configured", self.port_id));
        }

        self.handle.rxq_num = Some(rxq_num);
        self.handle.txq_num = Some(txq_num);

        self.dev_conf.rx_adv_conf.rss_conf = rte_eth_rss_conf {
            rss_key: null_mut(),
            rss_key_len: 0,
            rss_hf: if rxq_num > 1 {
                RTE_ETH_RSS_IP as u64 & self.dev_info.flow_type_rss_offloads
            } else { 0 },
            algorithm: 0 as rte_eth_hash_function,
        };

        let rc = unsafe {
            rte_eth_dev_configure(self.port_id, rxq_num, txq_num, &mut self.dev_conf)
        };
        if rc < 0 {
            return Err(format!("port-{}: rte_eth_dev_configure failed: {}", self.port_id, rc));
        }

        Ok(())
    }

    pub fn config_rxqs(&mut self, desc_num: u16, mempool: DpdkMempool) -> Result<(), String> {

        if self.rxqs.is_some() {
            return Err(format!("port-{}: Rx queues already set", self.port_id));
        }

        let q_num = self.handle.rxq_num.unwrap();
        let mut rxqs: Vec<RxqHandle> = Vec::with_capacity(q_num as usize);

        for qid in 0..q_num {
            rxqs.push(
                RxqHandle::new(
                    self.handle.clone(),
                    self.port_id, qid,
                    desc_num,
                    mempool.clone()
                ));
        }
        self.rxqs = Some(rxqs);
        Ok(())
    }

    pub fn config_txqs(&mut self, desc_num: u16) -> Result<(), String> {
        if self.txqs.is_some() {
            return Err(format!("port-{}: Tx queues already set", self.port_id));
        }

        let q_num = self.handle.txq_num.unwrap();
        let mut txqs: Vec<TxqHandle> = Vec::with_capacity(q_num as usize);

        for qid in 0..q_num {
            txqs.push(TxqHandle {
                _handle: self.handle.clone(),
                port_id: self.port_id,
                queue_id: qid,
                desc_num: desc_num,
            });
        }
        self.txqs = Some(txqs);

        Ok(())

    }

    pub fn config_rss(&mut self, hash_key: &str, hash_func: u64, hash_hfa: rte_eth_hash_function) -> Result<(), String> {
        if self.rxqs.is_some() {
            return Err(format!("port-{}: Rx queues already configured", self.port_id));
        }

        self.handle.rss_hash_key = Some(hash_key.to_string());
        self.handle.rss_hf = hash_func;
        self.handle.rss_hfa = hash_hfa;

        Ok(())
    }

    pub fn fetch_queues(&mut self) -> (Vec<RxqHandle>, Vec<TxqHandle>) {
        // call rte_eth_dev_start() here, then give ownership of Rxq/Txq to app
        (
            std::mem::take(&mut self.rxqs.as_mut().unwrap()),
            std::mem::take(&mut self.txqs.as_mut().unwrap())
        )
    }

    pub fn start(&mut self) -> Result<(), String> {
        unsafe {rte_eth_dev_start(self.port_id)};
        Ok(())
    }
}
