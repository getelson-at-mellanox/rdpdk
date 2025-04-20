use std::ptr::null_mut;
use rdpdk::lib::{MBuffMempoolHandle, NumaSocketId};
use rdpdk::lib::dpdk_raw::rte_mbuf::{rte_mbuf, RTE_MBUF_DEFAULT_BUF_SIZE};
use rdpdk::lib::eal::Eal;

const DEFAULT_RXQ_DESC_NUM: u16 = 64;
const DEFAULT_TXQ_DESC_NUM: u16 = 64;

fn main() {
    let mut eal = Eal::init().expect("Failed to init EAL");
    let mut ports = eal.take_eth_ports().expect("Failed to take ports");

    let mut port = ports.remove(0);

    let mut rx_mbuff_pool_handle =
        MBuffMempoolHandle::new("Rx echo pool", 1024)
            .data_root_size(RTE_MBUF_DEFAULT_BUF_SIZE as _)
            .socket(NumaSocketId::NumaSocketIdPort(port.id()).to_socket_id()).clone();
    
    let rxq_mempool = rx_mbuff_pool_handle.mempool_create()
        .expect("Failed to create mempool");

    port.configure(1, 1).expect(&format!("port-{}: Failed to configure", port.id()));

    port.config_rxqs(DEFAULT_RXQ_DESC_NUM, rxq_mempool)
        .expect(&format!("port-{}: Failed to set rxqs", port.id()));

    port.config_txqs(DEFAULT_TXQ_DESC_NUM)
        .expect(&format!("port-{}: Failed to set txqs", port.id()));

    let (mut rxqs, mut txqs) = port.fetch_queues();

    let rxq_a = rxqs.remove(0);
    let txq_a = txqs.remove(0);

    let _ = port.start();
    
    let io_thread = std::thread::spawn(move || {
        let mut rxq = rxq_a.activate();
        let mut txq = txq_a.activate();

        println!("=== [IO]: echo server started");

        let mbufs:[*mut rte_mbuf; 64] = [null_mut(); 64];
        loop {
            let rx_num = rxq.rx_burst(&mbufs);
            if rx_num > 0 {
                println!("=== [IO] echo server received {} packets", rx_num);
                txq.tx_burst(&mbufs[..rx_num as usize]);
            }
        }
    });

    io_thread.join().expect("Failed to join io thread");
}