use std::env;
use rdpdk::lib::{MBuffMempoolHandle, NumaSocketId};
use rdpdk::lib::dpdk_raw::rte_mbuf::RTE_MBUF_DEFAULT_BUF_SIZE;
use rdpdk::lib::eal::Eal;

const DEFAULT_RXQ_DESC_NUM: u16 = 64;
const DEFAULT_TXQ_DESC_NUM: u16 = 64;

fn main() {
    let eal_arg = env::args()
        .fold(String::new(), |acc, next | acc + &format!(" {}", next));
    
    let mut eal = Eal::init(&eal_arg).expect("Failed to init EAL");
    let mut ports = eal.take_eth_ports().expect("Failed to take ports");
    
    let mut port = ports.remove(0);

    let rx_mbuff_pool_handle =
        MBuffMempoolHandle::new("Rx echo pool", 1024)
            .data_root_size(RTE_MBUF_DEFAULT_BUF_SIZE as _)
            .socket(NumaSocketId::NumaSocketIdPort(port.id()).to_socket_id()).clone();
    
    port.configure(1, 1).expect(&format!("port-{}: Failed to configure", port.id()));
    
    port.config_rxqs(DEFAULT_RXQ_DESC_NUM, rx_mbuff_pool_handle)
        .expect(&format!("port-{}: Failed to set rxqs", port.id()));
    
    port.config_txqs(DEFAULT_TXQ_DESC_NUM)
        .expect(&format!("port-{}: Failed to set txqs", port.id()));
    
    let (mut rxqs, mut txqs) = port.start();
    
    let rxq_a = rxqs.remove(0);
    let txq_a = txqs.remove(0);

    std::thread::spawn(move || {
        let rxq = rxq_a.activate().expect("Failed to enable Rx polling");
        let txq = txq_a.activate().expect("Failed to enable Tx polling");
        
    });
}