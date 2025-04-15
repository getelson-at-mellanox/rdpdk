
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use crate::port::DpdkPort;
use crate::port::raw_port::PortParams;

pub type PciVendor = u16;
pub type PciDevice = u16;

pub type PciPortInitFn = fn(port_id: u16, device: PciDevice, port_params: Arc<PortParams>) -> Result<Box<dyn DpdkPort>, String>;

pub type PortHwMap = HashMap<PciVendor, PciPortInitFn>;

pub static KNOWN_PORTS: Lazy<Mutex<PortHwMap>> =
    Lazy::new(|| Mutex::new(HashMap::new()));


