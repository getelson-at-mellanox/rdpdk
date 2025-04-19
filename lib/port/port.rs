use std::sync::Arc;
use crate::lib::{MBuffMempoolHandle, NumaSocketId};
use crate::lib::dpdk_raw::rte_ethdev::{
    rte_eth_conf,
    rte_eth_hash_function,
    rte_eth_rxconf,
    rte_eth_txconf,
    rte_eth_dev_configure,
    rte_eth_rx_queue_setup,
    rte_eth_tx_queue_setup,
    rte_eth_hash_function_RTE_ETH_HASH_FUNCTION_DEFAULT,
};

#[derive(Debug, Clone)]
#[allow(unused)]
pub(crate) struct PortHandle {
    rxq_num: Option<u16>,
    txq_num: Option<u16>,
    rss_hash_key: Option<String>,
    rss_hf: u64, // see rte_ethdev::RTE_ETH_RSS_*
    rss_hfa: rte_eth_hash_function,
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

impl PortHandle {
    fn new() -> Self {
        PortHandle {
            rxq_num: None,
            txq_num: None,
            rss_hash_key: None,
            rss_hf: 0,
            rss_hfa: rte_eth_hash_function_RTE_ETH_HASH_FUNCTION_DEFAULT,
            // tx_conf: unsafe { std::mem::zeroed() },
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
    handle: PortHandle,
    port_id: u16,
    queue_id: u16,
    desc_num: u16,
}

impl TxqHandle {
    pub fn activate(mut self) -> Result<Txq, String> {
        let mut tx_conf: rte_eth_txconf = unsafe { std::mem::zeroed() };

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
            tx_conf:tx_conf,
            _phantom: std::marker::PhantomData,
        })
    }
}

// Handle allows moving between threads, its not polling!
#[derive(Debug)]
pub struct RxqHandle {
    handle: PortHandle,
    port_id: u16,
    queue_id: u16,
    desc_num: u16,
    mempool_handle: MBuffMempoolHandle,
}

impl RxqHandle {
    pub(crate) fn new(
        handle: PortHandle,
        port_id: u16,
        queue_id: u16,
        desc_num: u16,
        mempool_handle: MBuffMempoolHandle) -> Self {
        RxqHandle {
            handle: handle,
            port_id: port_id,
            queue_id: queue_id,
            desc_num: desc_num,
            mempool_handle: mempool_handle
        }
    }

    // This function is the key to the API design: it ensures the rx_burst()
    // function is only available via the Rxq struct, after enable_polling() has been called.
    // It "consumes" (takes "self" as a parameter, not a '&' reference!) which essentially
    // destroys/invalidates the handle from the Application level code.

    // It returns an Rxq instance, which has the PhantomData to encode the threading requirements,
    // and the Rxq has the rx_burst() function: this allows the application to recieve packets.
    pub fn activate(mut self) -> Result<Rxq, String> {
        let mempool = self.mempool_handle.mempool_create().unwrap();

        let mut rx_conf: rte_eth_rxconf = unsafe { std::mem::zeroed() };

        let rc = unsafe {
            rte_eth_rx_queue_setup(
                self.port_id,
                self.queue_id,
                self.desc_num,
                NumaSocketId::NumaSocketIdPort(self.port_id).to_socket_id() as _,
                &rx_conf as *const _,
                mempool.pool_ptr as *mut _
            )
        };
        if rc < 0 {
            return Err(format!("port-{}: rte_eth_rx_queue_setup failed: {}", self.port_id, rc));
        }

        Ok(Rxq {
            handle: self,
            rx_conf:rx_conf,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[derive(Debug)]
pub struct Rxq {
    handle: RxqHandle,
    rx_conf: rte_eth_rxconf,
    // This "PhantomData" tells the rust compiler to Pretend the Rc<()> is in this struct
    // but in practice it is a Zero-Sized-Type, so takes up no space. It is a compile-time
    // language technique to ensure the struct is not moved between threads. This encodes
    // the API requirement "don't poll from multiple threads without synchronisation (e.g. Mutex)"
    _phantom: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Rxq {
    // TODO: datapath Error types should be lightweight, not String. Here we return ().
    pub fn rx_burst(&mut self, _mbufs: &mut [u8]) -> Result<usize, ()> {
        // TODO: Design the Mbuf struct wrapper, and how to best return a batch
        //  e.g.: investigate "ArrayVec" crate for safe & fixed sized, stack allocated arrays
        //
        // There is work to do here, but I want to communicate the general DPDK/EAL/Eth/Rxq concepts
        // now, this part is not done yet: it is likely the hardest/most performance critical.
        //
        // call rte_eth_rx_burst() here
        println!(
            "[thread: {:?}] rx_burst: port {} queue {}",
            std::thread::current().id(),
            self.handle.port_id,
            self.handle.queue_id
        );
        Ok(0)
    }
}

pub struct Txq {
    handle: TxqHandle,
    tx_conf: rte_eth_txconf,
    _phantom: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[derive(Debug)]
pub struct Port {
    port_id: u16,
    handle: PortHandle,
    
    rxqs: Option<Vec<RxqHandle>>,
    txqs: Option<Vec<TxqHandle>>,

    dev_conf: rte_eth_conf,
}

pub type PortId = u16;
impl Port {
    pub fn id(&self) -> PortId {
        self.port_id
    }

    pub fn from_port_id(port_id: PortId) -> Self {
        Port {
            handle: PortHandle::new(),
            port_id:port_id,
            rxqs: None,
            txqs: None,
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

        let rc = unsafe {
            rte_eth_dev_configure(self.port_id, rxq_num, txq_num, &mut self.dev_conf)
        };
        if rc < 0 {
            return Err(format!("port-{}: rte_eth_dev_configure failed: {}", self.port_id, rc));
        }

        Ok(())
    }

    pub fn config_rxqs(&mut self, desc_num: u16, mempool_handle: MBuffMempoolHandle) -> Result<(), String> {

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
                    mempool_handle.clone()
                ));
        }
        self.rxqs = Some(rxqs);
        println!("{:?}", self.handle);
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
                handle: self.handle.clone(),
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

    pub fn start(&mut self) -> (Vec<RxqHandle>, Vec<TxqHandle>) {
        // call rte_eth_dev_start() here, then give ownership of Rxq/Txq to app
        (
            std::mem::take(&mut self.rxqs.as_mut().unwrap()),
            std::mem::take(&mut self.txqs.as_mut().unwrap())
        )
    }
}
