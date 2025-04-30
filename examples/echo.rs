use std::ptr::{null_mut};
use std::sync::mpsc;
use std::thread::JoinHandle;
use rdpdk::lib::{MBuffMempoolHandle, NumaSocketId};
use rdpdk::lib::dpdk_raw::rte_ethdev::{
    rte_flow, rte_flow_action, 
    rte_flow_action_queue, 
    rte_flow_attr,
    rte_flow_item,
    rte_flow_create,
    rte_flow_action_type_RTE_FLOW_ACTION_TYPE_END,
    rte_flow_action_type_RTE_FLOW_ACTION_TYPE_QUEUE,
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END, 
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_ETH, 
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_IPV4, 
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_TCP, 
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_UDP
};
use rdpdk::lib::dpdk_raw::rte_mbuf::{rte_mbuf, RTE_MBUF_DEFAULT_BUF_SIZE};
use rdpdk::lib::eal::Eal;
use rdpdk::lib::port::{RxqHandle, TxqHandle};
use std::thread;
use std::time::Duration;
use tokio::sync::broadcast;


const DEFAULT_RXQ_DESC_NUM: u16 = 64;
const DEFAULT_TXQ_DESC_NUM: u16 = 64;

#[derive(Debug, Clone)]
enum CtrlMsg {
    Stop,
}

#[derive(Debug)]
enum StatusMsg {
    Ready(u16),
    RxCnt((u16, u16))
}

macro_rules! spawn_io_thread {
    ($targ:expr) => {
        std::thread::spawn(move || {
            do_io($targ)
        })
    };
}

#[tokio::main]
async fn main() {
    let mut eal = Eal::init().await.expect("Failed to init EAL");
    let mut ports = eal.take_eth_ports().expect("Failed to take ports");

    let mut port = ports.remove(0);

    let socket_id = NumaSocketId::NumaSocketIdPort(port.id()).to_socket_id();
    let mut rx_mbuff_pool_handle = MBuffMempoolHandle::new("Rx echo pool", 1024);
    rx_mbuff_pool_handle.data_root_size(RTE_MBUF_DEFAULT_BUF_SIZE as _);
    rx_mbuff_pool_handle.socket(socket_id);
    let rxq_mempool = rx_mbuff_pool_handle.mempool_create()
        .expect("Failed to create mempool");

    let queues_num:u16 = 2;
    port.configure(queues_num, queues_num)
        .expect(&format!("port-{}: Failed to configure", port.id()));

    port.config_rxqs(DEFAULT_RXQ_DESC_NUM, rxq_mempool)
        .expect(&format!("port-{}: Failed to set rxqs", port.id()));

    port.config_txqs(DEFAULT_TXQ_DESC_NUM)
        .expect(&format!("port-{}: Failed to set txqs", port.id()));

    let _ = port.start();
    
    add_flows().expect("Failed to add flows");
    
    let (mut rxqs, mut txqs) = port.fetch_queues();

    let (status_tx, status_rx) = mpsc::channel::<StatusMsg>();
    let (control_tx, _) = broadcast::channel::<CtrlMsg>(queues_num as usize);
    
    let mut iotv: Vec<JoinHandle<()>> = Vec::new();
    for i in 0..queues_num {
        let rxqh = rxqs.remove(0);
        let txqh = txqs.remove(0);
        let status_tx = status_tx.clone();
        let control_tx = control_tx.clone();
        let iot_param: IoTParam = IoTParam(i, rxqh, txqh, status_tx, control_tx);
        let iot = spawn_io_thread!(iot_param);
        iotv.push(iot);
    }

    let mut total_rx_cnt: u16 = 0;
    loop {
        match status_rx.try_recv() {
            Ok(status) => match status {
                StatusMsg::Ready(id) => {
                    println!("queue-{}: is ready for IO", id);
                },
                StatusMsg::RxCnt((id, rx_cnt)) => {
                    println!("queue-{id}: received {rx_cnt} total {total_rx_cnt}");
                    total_rx_cnt += rx_cnt;
                    if total_rx_cnt > 10 {
                        println!("stop IO");
                        control_tx.send(CtrlMsg::Stop).unwrap();
                        break;
                    }
                }
            },
            Err(mpsc::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                println!("queue disconnected");
                break
            }
        }
    }
    
    loop {
        match port.stop() {
            Ok(_) => {
                println!("port-{} stopped", port.id());
                break
            },
            Err(q_cnt) => {
                println!("port={} cannot stop yet: {}", port.id(), q_cnt);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[derive(Debug)]
struct IoTParam(
    u16,
    RxqHandle,
    TxqHandle,
    mpsc::Sender<StatusMsg>,
    broadcast::Sender<CtrlMsg>
);

fn do_io(iotp: IoTParam) {
    
    let IoTParam(qid, rxqh, txqh, status_tx, control_tx) = iotp;
    
    let mut rxq = rxqh.activate();
    let mut txq = txqh.activate();
    
    let mut control_rx = control_tx.subscribe();
    status_tx.send(StatusMsg::Ready(qid)).unwrap();
    
    let mbufs:[*mut rte_mbuf; 64] = [null_mut(); 64];
    loop {
        
        match control_rx.try_recv() {
            Ok(CtrlMsg::Stop) => {
                println!("queue-{qid}: got stop command");
                return
            },
            Err(broadcast::error::TryRecvError::Empty) => {},
            Err(_) => return,
        }
        
        let rx_num = rxq.rx_burst(&mbufs);
        if rx_num > 0 {
            status_tx.send(StatusMsg::RxCnt((qid, rx_num))).unwrap();
            txq.tx_burst(&mbufs[..rx_num as usize]);
        }
    }
}

fn add_flows() -> Result<(), String>{
    
    let mut attr: rte_flow_attr = { unsafe { std::mem::zeroed() } };
    attr.set_ingress(1);
    
    let mut pattern_udp: [rte_flow_item;4] = { unsafe { std::mem::zeroed() } };
    pattern_udp[0].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_ETH;
    pattern_udp[1].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_IPV4;
    pattern_udp[2].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_UDP;
    pattern_udp[3].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END;

    let mut pattern_tcp: [rte_flow_item;4] = { unsafe { std::mem::zeroed() } };
    pattern_tcp[0].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_ETH;
    pattern_tcp[1].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_IPV4;
    pattern_tcp[2].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_TCP;
    pattern_tcp[3].type_ = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END;
    
    let queue_udp: rte_flow_action_queue = rte_flow_action_queue { index: 0 };
    let queue_tcp: rte_flow_action_queue = rte_flow_action_queue { index: 1 };
    
    let mut actions_udp: [rte_flow_action;2] = { unsafe { std::mem::zeroed() } };
    actions_udp[0].conf = &queue_udp as *const _ as *mut _;
    actions_udp[0].type_ = rte_flow_action_type_RTE_FLOW_ACTION_TYPE_QUEUE;
    actions_udp[1].type_ = rte_flow_action_type_RTE_FLOW_ACTION_TYPE_END;

    let mut actions_tcp: [rte_flow_action;2] = { unsafe { std::mem::zeroed() } };
    actions_tcp[0].conf = &queue_tcp as *const _ as *mut _;
    actions_tcp[0].type_ = rte_flow_action_type_RTE_FLOW_ACTION_TYPE_QUEUE;
    actions_tcp[1].type_ = rte_flow_action_type_RTE_FLOW_ACTION_TYPE_END;
    
    let udp_flow: *mut rte_flow = unsafe { 
        rte_flow_create(
            0u16,
            &attr as *const _ as *mut _, 
            &pattern_udp  as *const _ as *mut _, 
            &actions_udp as *const _ as *mut _,
            null_mut()
        ) 
    };
    if udp_flow.is_null() {
        return Err("Failed to create udp flow".to_string());
    }

    let tcp_flow: *mut rte_flow = unsafe {
        rte_flow_create(
            0u16,
            &attr as *const _ as *mut _,
            &pattern_tcp  as *const _ as *mut _,
            &actions_tcp as *const _ as *mut _,
            null_mut()
        )
    };
    
    if tcp_flow.is_null() {
        return Err("Failed to create tcp flow".to_string());
    }
    
    Ok(())
}