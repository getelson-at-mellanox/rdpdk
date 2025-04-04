#[path = "actions/actions.rs"]
pub mod actions;
#[path = "attr/attr.rs"]
pub mod attr;
#[path = "items/items.rs"]
pub mod items;


use actions::FlowActions;
use attr::FlowAttributes;
use items::FlowItems;
use crate::cmdline::ModuleOps;
use crate::dpdk_raw::rte_ethdev::{rte_flow_create, rte_flow_error};
use std::collections::HashMap;
use std::mem;
use std::str::FromStr;

type CmdMap = HashMap<String, Box<dyn CmdOps>>;

pub struct FlowCmd {
    commands: CmdMap,
}

unsafe impl Send for FlowCmd {}
unsafe impl Sync for FlowCmd {}

impl FlowCmd {
    pub fn new() -> Self {
        let mut map = FlowCmd {
            commands: CmdMap::new(),
        };
        map.commands.insert("create".to_string(), CreateCmd::new());
        map
    }
}

impl ModuleOps for FlowCmd {
    fn parse_cmd(&self, input: &mut Vec<String>) {
        input.remove(0); // flow
        loop {
            match self.commands.get(&input[0]) {
                None => return,
                Some(op) => op.parse(input),
            }
            if input.len() == 0 {
                return;
            }
        }
    }
}

struct CreateCmd;
impl CreateCmd {
    pub fn new() -> Box<dyn CmdOps> {
        Box::new(CreateCmd)
    }
}

impl CmdOps for CreateCmd {
    fn name(&self) -> &str {
        "create"
    }
    fn parse(&self, input: &mut Vec<String>) {
        let port = u16::from_str(&input[1]).unwrap();

        let mut attr: FlowAttributes = FlowAttributes::new();
        input.remove(0); // create
        input.remove(0); // <port>
        attr.parse_attr(input);

        input.remove(0); // pattern

        let mut items: FlowItems = FlowItems::new();
        items.parse_pattern(input);

        input.remove(0); // actions

        let mut actions: FlowActions = FlowActions::new();
        actions.parse_actions(input);

        let mut flow_error: rte_flow_error = unsafe { mem::zeroed() };
        let err_ptr: *mut rte_flow_error = &mut flow_error;

        let flow = unsafe {
            rte_flow_create(
                port,
                attr.get_raw_attr(),
                items.get_raw_pattern(),
                actions.get_raw_actions(),
                err_ptr,
            )
        };

        if flow as u64 != 0 {
            println!("Flow created");
        } else {
            println!("Flow failed");
        }
    }
}

pub trait CmdOps: Send + Sync {
    fn name(&self) -> &str;

    fn parse(&self, input: &mut Vec<String>);
}

