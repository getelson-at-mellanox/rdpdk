use std::env;
use std::ffi::{c_char, c_uint, CString};
use std::fmt::Debug;
use std::sync::Arc;
use std::thread::{park, JoinHandle};
use clap::{Command, Arg, ArgMatches};
use tokio::sync::Mutex;
use crate::lib::port::*;
use crate::lib::dpdk_raw::rte_eal::{rte_eal_cleanup, rte_eal_init};
use crate::lib::dpdk_raw::rte_ethdev::{rte_eth_dev_count_avail, rte_get_next_lcore, rte_lcore_count, RTE_MAX_LCORE};

#[derive(Debug)]
pub struct Eal {
    eth_ports: Option<Vec<Port>>,
    rdpdk: Rdpdk,
}

impl Eal {
    //  allow init once,
    pub async fn init() -> Result<Self, String> {

        let rdpdk_inner = if let Some((eal_args, rdpdk_args, _app_args)) = parse_command_line() {
            let mut argv:Vec<*mut c_char> = eal_args
                .iter()
                .map(|arg| CString::new(arg.as_bytes()).unwrap().into_raw())
                .collect();

            let rc = unsafe { rte_eal_init(argv.len() as i32, argv.as_mut_ptr()) };
            if rc < 0 {
                unsafe {rte_eal_cleanup()};
                return Err(format!("rte_eal_init failed with rc {}", rc));
            }

            let rdpdk: Rdpdk = match rdpdk_init(rdpdk_args).await {
                Ok(inner) => inner,
                Err(err) => {
                    unsafe {rte_eal_cleanup()};
                    return Err(err);
                }
            };

            rdpdk
        } else {
            return Err("Bad command line arguments".to_string());
        };

        let ports_num = unsafe { rte_eth_dev_count_avail() };
        
        println!("=== [EAL]: ports_num: {}", ports_num);
        
        if ports_num == 0 {
            return Err("No ports found".to_string());       
        }
        
        let mut eth_ports = Vec::with_capacity(ports_num as usize);
        for port_id in 0..ports_num {
            eth_ports.push(Port::from_port_id(port_id))
        }

        Ok(Eal {
            eth_ports: Some(eth_ports),
            rdpdk: rdpdk_inner,
        })
    }

    // API to get eth ports, taking ownership. It can be called once.
    // The return will be None for future calls
    pub fn take_eth_ports(&mut self) -> Option<Vec<Port>> {
        self.eth_ports.take()
    }

    pub async fn rdpdk_set_worker(&mut self, lcore:u32, worker: Box<dyn LcoreWorker>) {
        self.rdpdk.lcores[lcore as usize].lock().await.worker = Some(worker);
        let handle = &self.rdpdk.handles[lcore as usize];
        if let Some(handle) = handle {
            handle.thread().unpark();       
        }
    }


    pub fn lcores_iter(&self) -> LcoresIterator {
        LcoresIterator {
            cores: &self.rdpdk.cores,
            index: 0,
        }
    }

    pub fn lcores_iter_mut(&mut self) -> LcoresIterator {
        LcoresIterator {
            cores: &self.rdpdk.cores,
            index: 0,
        }
    }
}

impl Drop for Eal {
    fn drop(&mut self) {
        unsafe { rte_eal_cleanup() };
    }
}

/// Command line format:
/// APPLICATION_NAME [EAL parameters] -- [RDPDK parameters] -- [application parameters]
async fn rdpdk_init(args: Vec<String>) -> Result<Rdpdk, String> {
    let cmd_opt = Command::new("rdpdk")
        .about("rdpdk command line options")
        .no_binary_name(true)
        .arg(
            // RDPDK can define dedicated workers cores set
            // If not set, EAL cores are used
            Arg::new("cores list")
                .short('l')
                .long("core-list")
                .help("Comma separated cores list. Supports inclusive range `c1-cX`")
                .required(false)
        );

    let opt_matches = cmd_opt.get_matches_from(args);
    let mut inner = Rdpdk::new();
    inner.cores = parse_core_opt(&opt_matches);
    inner.start_lcores(inner.cores.clone()).await;

    Ok(inner)
}

pub trait LcoreWorker : Debug + Send + Sync {
    fn run(&mut self, core_id: u32);
}

#[derive(Debug)]
struct Lcore {
    worker: Option<Box<dyn LcoreWorker>>,
}

async fn do_lcore(core_id: u32, lcore: Arc<Mutex<Lcore>>) {
    loop {
        park();
        if let Some(ref mut worker) = lcore.lock().await.worker.take() {
            worker.run(core_id);
        }
    }
}

impl Lcore {
    fn new() -> Self {
        Self {
            worker: None,
        }
    }
}

pub struct LcoresIterator<'a> {
    cores: &'a Vec<u32>,
    index: usize,
}

impl<'a> Iterator for LcoresIterator<'a> {
    type Item = &'a u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.cores.len() {
            let ret = Some(&self.cores[self.index]);
            self.index += 1;
            ret
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct Rdpdk {
    handles: [Option<JoinHandle<()>>; RTE_MAX_LCORE as usize],
    lcores: [Arc<Mutex<Lcore>>; RTE_MAX_LCORE as usize],
    cores: Vec<u32>,
}

impl Rdpdk {
    fn new() -> Self {
        Self {
            handles: std::array::from_fn(|_| None),
            lcores: std::array::from_fn(|_| Arc::new(Mutex::new(Lcore::new()))),
            cores: Vec::new(),
        }
    }

    async fn start_lcores(&mut self, cores: Vec<u32>) {
        for core in cores {
            let lcore = self.lcores[core as usize].clone();
            self.handles[core as usize] = Some(std::thread::spawn(move || {
                tokio::runtime::Runtime::new().unwrap().block_on(async move {
                    do_lcore(core as u32, lcore).await;
                })
            }));
        }
    }
}

fn parse_command_line() -> Option<(Vec<String>, Vec<String>, Vec<String>)>  {
    let args = env::args().collect::<Vec<String>>();
    let mut eal_args:Vec<String> = Vec::new();
    let mut rdpdk_args:Vec<String> = Vec::new();
    let mut app_args:Vec<String> = Vec::new();

    const EAL_ARG: usize = 0;
    const RDPDK_ARG: usize = 1;
    const APP_ARG: usize = 2;

    let mut arg_type = EAL_ARG;

    for arg in args {
        if arg.eq("--") { arg_type += 1; }
        else {
            match arg_type {
                EAL_ARG => eal_args.push(arg),
                RDPDK_ARG => rdpdk_args.push(arg),
                APP_ARG => app_args.push(arg),
                _ => { return None },
            }
        }
    }
    Some((eal_args, rdpdk_args, app_args))
}

fn parse_core_opt(opt_matches: &ArgMatches) -> Vec<u32> {
    let cores:Vec<u32> = if let Some(cores_list) = opt_matches.get_one::<String>("cores list") {
        let mut cores:Vec<u32> = Vec::with_capacity(unsafe { rte_lcore_count() } as usize);

        cores_list.split(',')
            .for_each(|element| {
                if element.contains('-') {
                    let range = element.split('-')
                        .map(|x| x.parse::<u32>().unwrap())
                        .collect::<Vec<u32>>();
                    for i in range[0]..range[1] + 1 { cores.push(i); }
                } else {
                    let core = element.parse::<u32>().unwrap();
                    cores.push(core);
                }
            });
        cores
    } else {
        let mut cores:Vec<u32> = Vec::with_capacity(unsafe { rte_lcore_count() } as usize);
        let mut c = u32::max_value();

        loop {
            c = unsafe { rte_get_next_lcore(c as c_uint, 0, 0) };
            if c < RTE_MAX_LCORE { cores.push(c); } else { break }
        }
        cores
    };
    println!("=== RDPDK cores: {:?}", cores);
    cores
}
