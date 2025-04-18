#[path = "cmd_module/cmd_module.rs"]
pub mod cmd_module;

use cmd_module::flow::FlowCmd;
use cmd_module::port::PortModule;
use rdpdk::dpdk_raw::rte_eal::{rte_eal_cleanup, rte_eal_init};
use std::collections::HashMap;
use std::{env, slice};
use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::raw::{c_char, c_int};
use std::ptr::null_mut;
use rdpdk::dpdk_raw::rte_ethdev::{rte_eth_dev_count_avail, rte_eth_dev_get_name_by_port, rte_eth_dev_socket_id, rust_get_port_eth_device, RTE_ETH_NAME_MAX_LEN};
use rdpdk::dpdk_raw::rte_mbuf::{rte_mbuf};
use rdpdk::dpdk_raw::rte_mbuf_core::RTE_MBUF_DEFAULT_BUF_SIZE;
use std::thread;

use rdpdk::port::{alloc_mbuf_pool, DpdkPort, DpdkPortConf};
use rdpdk::port::raw_port::{RawDpdkPort};
use std::sync::{Arc, Mutex};

use rdpdk::port::init::{
    PciVendor,
    PciDevice,
    KNOWN_PORTS,
};
use crate::cmd_module::CmdModuleOps;

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

#[derive(Copy, Clone)]
struct MbufPtr(*const rte_mbuf);

impl std::fmt::Debug for MbufPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_null() {
            return write!(f, "MbufPtr(null)");
        }

        // Safety: We've checked that the pointer is not null
        let mbuf = unsafe { &*self.0 };

        let data_off = unsafe { mbuf.__bindgen_anon_1.__bindgen_anon_1.data_off };
        let pkt_len = unsafe { mbuf.__bindgen_anon_2.__bindgen_anon_1.pkt_len };

        write!(f,
            "MbufPtr {{ address: {:p}, data_offset: {}, packet_length: {} }}",
            self.0,
            data_off,
            pkt_len
        )
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

fn l2_addr_swap(mbuf: &mut rte_mbuf)
{
    let data_off = unsafe {mbuf.__bindgen_anon_1.__bindgen_anon_1.data_off};
    let raw_ptr = mbuf.buf_addr.wrapping_add(data_off as usize) as *mut u8;
    let eth: &mut [u8] = unsafe { slice::from_raw_parts_mut(raw_ptr, 14) };

    for i in 0..6 {
        let aux = eth[i];
        eth[i] = eth[i + 6];
        eth[i + 6] = aux;
    }
}

fn do_io(runpmd: Arc<RunPmd>) {
    for pretected in runpmd.ports.iter() {
        let rx_pkts:[*mut rte_mbuf; 64] = [null_mut(); 64];
        let mut port = pretected.lock().unwrap();
        let port_id = port.port_id();
        let rx_burst_res = port.rx_burst(port_id, &rx_pkts);

            match rx_burst_res {
                Err(err) => println!("{err}"),
                Ok(rx_num) => {
                    let tx_pool:&[*mut rte_mbuf] = &rx_pkts[0..rx_num as usize];
                    if rx_num > 0 {
                        for i in 0..rx_num {
                            show_packet(&unsafe { *(rx_pkts[i as usize] as *const rte_mbuf) });
                            let mbuf_ptr: *const rte_mbuf = rx_pkts[i as usize] as *const rte_mbuf;
                            println!("{:?}", MbufPtr(mbuf_ptr));

                            l2_addr_swap(&mut unsafe { *(rx_pkts[i as usize] as *mut rte_mbuf) });

                            let _ = port.tx_burst(port_id, tx_pool);
                        }
                        continue;
                    }
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

type CmdModule = HashMap<String, Box<dyn CmdModuleOps>>;
fn register_cmd_modules() -> CmdModule {
    let mut modules = CmdModule::new();
    modules.insert("flow".to_string(), Box::new(FlowCmd::new()));
    modules.insert("port".to_string(), Box::new(PortModule::new()));
    modules
}

fn query_port_businfo(port_id: u16) -> (PciVendor, PciDevice) {

    // businfo: "vendor_id=15b3, device_id=101d"

    let businfo = unsafe {
        let dev : &rdpdk::dpdk_raw::ethdev_driver::rte_eth_dev =
            & *(rust_get_port_eth_device(port_id)
                as *mut _ as *const _);
        let device = & *dev.device;
        let c_str = device.bus_info.cast::<c_char>();

        let str = CStr::from_ptr(c_str);
        str.to_string_lossy().into_owned()
    };

    println!("=== businfo: {}", &businfo);

    let vd:Vec<String> = businfo.split(", ")
        .map(|x| x.to_string())
        .collect();
    let vendor: Vec<&str> = vd[0].split("=")
        .collect();
    let device: Vec<&str> = vd[1].split("=")
        .collect();

    (u16::from_str_radix(vendor[1], 16).unwrap(),
        u16::from_str_radix(device[1], 16).unwrap())

}

fn start_port(port_id: u16, port_conf: &DpdkPortConf) -> Result<Box<dyn DpdkPort>, String>
{
    let (vendor, device) = query_port_businfo(port_id);

    for (v, func) in KNOWN_PORTS.lock().unwrap().iter() {
        if vendor != *v { continue }
            match func(port_id, device, port_conf) {
                Ok(port) => { return Ok(port); },
                Err(_e) => { continue; }
            }
    }
    println!("port {port_id}: fallback to raw port");
    Ok(Box::new(RawDpdkPort::init(port_id, port_conf).unwrap()))
}



fn init_ports_configuration(_args: &Vec<String>, port_num: u16) -> Vec::<DpdkPortConf> {
    let mut port_conf =
        Vec::<DpdkPortConf>::with_capacity(port_num as usize);

    let mbuf_pool = alloc_mbuf_pool(
        "runpmd_default_mbuf_pool",
        1024,
        0,
        0,
        RTE_MBUF_DEFAULT_BUF_SIZE as u16,
        0,
    ).unwrap() ;
    let mbuf_pool = Arc::new(mbuf_pool);

    for port_id in 0..port_num {
        let socket_id = unsafe { rte_eth_dev_socket_id(port_id) } as u32;
        let pc = DpdkPortConf::new_from(
            port_id,
            unsafe { std::mem::zeroed() },
            unsafe { std::mem::zeroed() },
            unsafe { std::mem::zeroed() },
            1,
            1,
            64,
            64,
            socket_id,
            socket_id,
            Some(mbuf_pool.clone())
        ).unwrap();
        port_conf.insert(port_id as usize, pc);
    }

    port_conf
}

struct RunPmd {
    ports: Vec<Mutex<Box<dyn DpdkPort>>>    
}
unsafe impl Send for RunPmd {}
unsafe impl Sync for RunPmd {}

fn main() {
    let (eal_params, app_params) = separate_cmd_line();

    let port_num = match eal_init(&eal_params) {
        Ok(n) => n,
        Err(e) => {
            println!("{e}");
            std::process::exit(255);
        }
    };

    let port_conf = Arc::new(init_ports_configuration(&app_params, port_num));

    let mut ports: Vec<Mutex<Box<dyn DpdkPort>>> = Vec::with_capacity(port_num as usize);
    for port_id in 0..port_num {
        match start_port(port_id, &port_conf[port_id as usize]) {
            Ok(p) => ports.push(Mutex::new(p)),
            Err(e) => {
                println!("{e}");
                std::process::exit(255);
            }
        }
    }
    show_ports_summary(&ports);
    
    let runpmd = Arc::new(RunPmd {
        ports: ports
    });
    

    let _ = thread::spawn(move || {
        loop { do_io(runpmd.clone()); }
    });

    let cli_thread = thread::spawn(move || {
        let modules = register_cmd_modules();
        run_interactive(&modules);
    });

    cli_thread.join().unwrap();

    // without direct reference to the mlx5 port linker does not include it in a build.
    mlx5::mlx5_pol(); // TODO: remove
}

pub fn show_ports_summary(ports: &Vec<Mutex<Box<dyn DpdkPort>>>) {
    let mut name_buf: [c_char; RTE_ETH_NAME_MAX_LEN as usize] =
        [0 as c_char; RTE_ETH_NAME_MAX_LEN as usize];
    let title = format!("{:<4}    {:<32} {:<14}", "Port", "Name", "Driver");
    println!("{title}");
    ports.iter().for_each(|p| unsafe {

        let summary = {
            let port = p.lock().unwrap();

            let _rc = rte_eth_dev_get_name_by_port(port.port_id(), name_buf.as_mut_ptr());
            let name = CStr::from_ptr(name_buf.as_ptr());
            let drv = CStr::from_ptr(port.port_conf().dev_info.driver_name);
            format!(
                "{:<4}    {:<32} {:<14}",
                port.port_id(),
                name.to_str().unwrap(),
                drv.to_str().unwrap()
            )
        };
        println!("{summary}");
    });
}
