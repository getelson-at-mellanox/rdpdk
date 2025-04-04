use rdpdk::cmdline::flow::FlowCmd;
use rdpdk::cmdline::ModuleOps;
use rdpdk::dpdk_raw::rte_eal::{rte_eal_cleanup, rte_eal_init};
use std::collections::HashMap;
use std::{env, slice};
use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr::null_mut;
use rdpdk::dpdk_raw::rte_ethdev::{
    rte_eth_conf,
    rte_eth_dev_count_avail,
    rte_eth_dev_get_name_by_port,
    rte_eth_dev_socket_id,
    rte_eth_rss_conf,
    rte_eth_rx_mq_mode_RTE_ETH_MQ_RX_RSS,
    rte_eth_rx_mq_mode_RTE_ETH_MQ_RX_VMDQ_DCB_RSS,
    rte_eth_rxconf,
    rte_eth_txconf,
    RTE_ETH_NAME_MAX_LEN,
    RTE_ETH_RSS_IP
};
use rdpdk::dpdk_raw::rte_mbuf::{rte_mbuf, rte_pktmbuf_free_bulk};
use rdpdk::dpdk_raw::rte_mbuf_core::RTE_MBUF_DEFAULT_BUF_SIZE;
use rdpdk::dpdk_raw::rte_mempool::rte_mempool;
use std::thread;

use rdpdk::port::{alloc_mbuf_pool, DpdkPortConf, DpdkPortCtrl, DpdkPortData};
use rdpdk::raw_port::RawDpdkPort;
use std::sync::Arc;
use rdpdk::cmdline::port::PortModule;

fn read_input() -> Result<String, String> {
    let mut buffer = String::new();

    print!(">>> ");
    std::io::stdout().flush().unwrap();

    let len = std::io::stdin().read_line(&mut buffer).unwrap();
    println!("=== input len: {len}");
    if len == 0 {
        return Ok(buffer);
    }

    if buffer.ends_with("\\\n") {
        loop {
            buffer.remove(buffer.len() - 1);
            buffer.remove(buffer.len() - 1);
            print!("> ");
            std::io::stdout().flush().unwrap();
            std::io::stdin().read_line(&mut buffer).unwrap();
            if !buffer.ends_with("\\\n") {
                break;
            }
        }
    }

    Ok(buffer)
}

fn separate_cmd_line() -> (Vec<String>, Vec<String>) {
    let argv = env::args().collect::<Vec<_>>();
    let mut eal_params = Vec::<String>::new();
    let mut app_params = Vec::<String>::new();
    let mut eal: bool = true;

    for arg in argv {
        if arg.eq("--") {
            eal = false;
            continue;
        } else if eal {
            eal_params.push(arg.clone());
        } else {
            app_params.push(arg.clone());
        }
    }
    (eal_params, app_params)
}

fn eal_init(args: &Vec<String>) -> Result<u16, String> {
    let mut argv: Vec<*mut c_char> = args
        .iter()
        .map(|arg| CString::new(arg.as_bytes()).unwrap().into_raw())
        .collect();

    let rc = unsafe { rte_eal_init(env::args().len() as c_int, argv.as_mut_ptr()) };
    if rc < 0 {
        unsafe {
            rte_eal_cleanup();
        }
        Err("faield to init eal".to_string())
    } else {
        Ok(unsafe { rte_eth_dev_count_avail() } as u16)
    }
}

fn run_command(modules: &CmdModule, command: &str) {
    let mut input = command
        .split_ascii_whitespace()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    match modules.get(&input[0]) {
        Some(op) => {
            op.parse_cmd(&mut input);
        }
        None => {
            println!("Unknown command: {}", &input[0]);
            ()
        }
    }
}

fn show_packet(mbuf: &rte_mbuf) {

    let data_off = unsafe {mbuf.__bindgen_anon_1.__bindgen_anon_1.data_off};
    let pkt_len = unsafe {mbuf.__bindgen_anon_2.__bindgen_anon_1.pkt_len};
    let raw_ptr = mbuf.buf_addr.wrapping_add(data_off as usize) as *const u8;
    let eth:&[u8] = unsafe { slice::from_raw_parts(raw_ptr, 14) };

    println!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} > \
        {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} eth_type {:x} len {}",
        eth[6],eth[7],eth[8],eth[9],eth[10],eth[11],
        eth[0],eth[1],eth[2],eth[3],eth[4],eth[5],
        u16::from_be_bytes([eth[12],eth[13]]) as u16,
        pkt_len
    );
}

fn do_rx(ports:&mut Vec<RawDpdkPort>) {
    for port in &mut *ports {

        let mut rx_pool:[*mut rte_mbuf; 64] = [null_mut(); 64];

        loop {
            match port.rx_burst(port.port_id, &mut rx_pool as *mut *mut _, 64u16) {
                Err(err) => println!("{err}"),
                Ok(rx_num) => {
                    if rx_num > 0 {
                        unsafe {rte_pktmbuf_free_bulk(
                            &mut rx_pool as *mut *mut _,
                            rx_num as c_uint)};
                        for i in 0..rx_num {
                            show_packet( & unsafe {*(rx_pool[i as usize] as *const rte_mbuf)});
                        }
                        continue;
                    }
                },
            }
        }
    }

}
fn run_interactive(modules: &CmdModule) {
    loop {
        let input = read_input().unwrap();
        if input.len() < 2 {
            continue;
        }

        let slin = &input[0..input.len() - 1];
        println!("===\n\'{}\': len:{}\n===", slin, slin.len());
        match slin {
            "exit" => break,
            _ => run_command(modules, slin),
        }
    }
    println!("Live long and prosper");
}

type CmdModule = HashMap<String, Box<dyn ModuleOps>>;
fn register_cmd_modules() -> CmdModule {
    let mut modules = CmdModule::new();
    modules.insert("flow".to_string(), Box::new(FlowCmd::new()));
    modules.insert("port".to_string(), Box::new(PortModule::new()));
    modules
}

fn start_port(port_id: u16, port_config: Arc<PortConfig>, pool: *mut rte_mempool) -> Result<RawDpdkPort, String> {
    let dev_conf: rte_eth_conf = unsafe { std::mem::zeroed() };
    let tx_conf: rte_eth_txconf = unsafe { std::mem::zeroed() };
    let rx_conf: rte_eth_rxconf = unsafe { std::mem::zeroed() };

    let mut port_conf = DpdkPortConf::new_from(
        port_id,
        dev_conf,
        tx_conf,
        rx_conf,
        1,
        1,
        64,
        64,
    ).unwrap();

    port_conf.dev_conf.rx_adv_conf.rss_conf = rte_eth_rss_conf {
        rss_key: null_mut(),
        rss_key_len: 0,
        rss_hf: if port_config.rxq_num > 1 {
            RTE_ETH_RSS_IP as u64 & port_conf.dev_info.flow_type_rss_offloads
        } else { 0 },
        algorithm: 0 as rdpdk::dpdk_raw::rte_ethdev::rte_eth_hash_function,
    };

    if port_conf.dev_conf.rx_adv_conf.rss_conf.rss_hf != 0 {
        port_conf.dev_conf.rxmode.mq_mode = rte_eth_rx_mq_mode_RTE_ETH_MQ_RX_VMDQ_DCB_RSS
            & rte_eth_rx_mq_mode_RTE_ETH_MQ_RX_RSS;
    }

    let mut port = RawDpdkPort::init(port_id, port_conf).unwrap();

    let socket_id = unsafe { rte_eth_dev_socket_id(port_id) } as u32;
    let _ = port.configure();
    let _ = port.config_txq(0, socket_id);
    let _ = port.config_rxq(0, pool as _, socket_id);
    let _ = port.start();

    Ok(port)
}

struct PortConfig {
    rxq_num: u16,
    _txq_num: u16,
}

fn init_port_configuration(_args: &Vec<String>) -> PortConfig {
    PortConfig {
        rxq_num: 1,
        _txq_num: 1,
    }
}

fn main() {
    let (eal_params, app_params) = separate_cmd_line();

    let port_num = match eal_init(&eal_params) {
        Ok(n) => n,
        Err(e) => {
            println!("{e}");
            std::process::exit(255);
        }
    };

    let port_config = Arc::new(
        init_port_configuration(&app_params));

    let mbuf_pool: *mut rte_mempool = alloc_mbuf_pool(
        "runpmd_default_mbuf_pool",
        1024,
        0,
        0,
        RTE_MBUF_DEFAULT_BUF_SIZE as u16,
        0,
    ).unwrap() as *mut rte_mempool;

    let mut ports: Vec<RawDpdkPort> = Vec::new();
    for port_id in 0..port_num {
        match start_port(port_id, port_config.clone(), mbuf_pool) {
            Ok(p) => ports.push(p),
            Err(e) => {
                println!("{e}");
                std::process::exit(255);
            }
        }
    }
    show_ports_summary(&ports);

    let _ = thread::spawn(move || {
        do_rx(&mut ports);
    });

    let cli_thread = thread::spawn(move || {
        let modules = register_cmd_modules();
        run_interactive(&modules);
    });

    cli_thread.join().unwrap();
}

pub fn show_ports_summary(ports: &Vec<RawDpdkPort>) {
    let mut name_buf: [c_char; RTE_ETH_NAME_MAX_LEN as usize] =
        [0 as c_char; RTE_ETH_NAME_MAX_LEN as usize];
    let title = format!("{:<4}    {:<32} {:<14}", "Port", "Name", "Driver");
    println!("{title}");
    ports.iter().for_each(|p| unsafe {
        let _rc = rte_eth_dev_get_name_by_port(p.port_id, name_buf.as_mut_ptr());
        let name = CStr::from_ptr(name_buf.as_ptr());
        let drv = CStr::from_ptr(p.port_conf.dev_info.driver_name);
        let summary = format!(
            "{:<4}    {:<32} {:<14}",
            p.port_id,
            name.to_str().unwrap(),
            drv.to_str().unwrap()
        );
        println!("{summary}");
    });
}
