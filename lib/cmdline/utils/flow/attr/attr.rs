use crate::cmdline::arg::Arg;
use crate::cmdline::arg::arg_int::ArgInt;
use crate::cmdline::param::Param;
use crate::dpdk_raw::rte_ethdev::{rte_flow_attr};
use std::collections::HashMap;
use std::mem;

pub enum Domain {
    Ingress,
    Egress,
    Transfer,
}

pub struct AttrContext {
    pub group: Option<u32>,
    pub priority: Option<u32>,
    pub domain: Option<Domain>,
    raw_attr: rte_flow_attr,
}

impl AttrContext {
    fn new() -> AttrContext {
        AttrContext {
            group: None,
            priority: None,
            domain: None,
            raw_attr: unsafe { mem::zeroed() },
        }
    }

    fn build_raw_attr(&mut self) {
        if self.group.is_some() {
            self.raw_attr.group = self.group.unwrap();
        }
        if self.priority.is_some() {
            self.raw_attr.priority = self.priority.unwrap();
        }
        match self.domain {
            Some(Domain::Ingress) => self.raw_attr.set_ingress(1),
            Some(Domain::Egress) => self.raw_attr.set_egress(1),
            Some(Domain::Transfer) => self.raw_attr.set_transfer(1),
            None => (),
        }
    }
}

trait AttrOps {
    fn name(&self) -> &str;
    fn parse_attr(&self, input: &mut Vec<String>, context: &mut AttrContext);
}

type AttrMap = HashMap<String, Box<dyn AttrOps>>;
pub struct FlowAttributes {
    map: AttrMap,
    context: AttrContext,
}

impl FlowAttributes {
    pub fn new() -> FlowAttributes {
        let mut attr = FlowAttributes {
            map: HashMap::new(),
            context: AttrContext::new(),
        };

        for obj in [
            Ingress::new(),
            Egress::new(),
            Transfer::new()] {
            attr.map.insert(obj.name().to_string(), obj);
        }

        create_attribute("group", &mut attr.map);
        create_attribute("priority", &mut attr.map);

        attr
    }

    pub fn parse_attr(&mut self, input: &mut Vec<String>) {
        loop {
            match self.map.get(&input[0]) {
                None => {
                    self.context.build_raw_attr();
                    return;
                }
                Some(op) => {
                    op.parse_attr(input, &mut self.context);
                }
            }
        }
    }

    pub fn get_raw_attr(&self) -> *const rte_flow_attr {
        &self.context.raw_attr as *const rte_flow_attr
    }
}

impl AttrOps for Param {
    fn name(&self) -> &str {
        &self.name
    }
    fn parse_attr(&self, input: &mut Vec<String>, context: &mut AttrContext) {
        let arg = ArgInt::<u32>::new().strton(&input[1]).unwrap();

        match input[0].as_str() {
            "group" => {
                context.group = Some(arg);
            }
            "priority" => {
                context.priority = Some(arg);
            }
            _ => return,
        }

        input.remove(0);
        input.remove(0);
    }
}

fn create_attribute(attr_name: &str, map: &mut AttrMap) {
    let arg: Box<dyn Arg> = Box::new(ArgInt::<u32>::new());

    let op = Box::new(Param::from(attr_name, None, None, Some(vec![(arg, 0)])));
    map.insert(attr_name.to_string(), op);
}

struct Ingress;
impl Ingress {
    fn new() -> Box<dyn AttrOps> {
        Box::new(Ingress)
    }
}

impl AttrOps for Ingress {
    fn name(&self) -> &str {
        "ingress"
    }
    fn parse_attr(&self, input: &mut Vec<String>, context: &mut AttrContext) {
        context.domain = Some(Domain::Ingress);
        input.remove(0);
    }
}

struct Egress;

impl Egress {
    fn new() -> Box<Egress> {
        Box::new(Egress)
    }
}

impl AttrOps for Egress {
    fn name(&self) -> &str {
        "egress"
    }
    fn parse_attr(&self, input: &mut Vec<String>, context: &mut AttrContext) {
        context.domain = Some(Domain::Egress);
        input.remove(0);
    }
}

struct Transfer;
impl Transfer {
    fn new() -> Box<Transfer> {
        Box::new(Transfer)
    }
}
impl AttrOps for Transfer {
    fn name(&self) -> &str {
        "transfer"
    }
    fn parse_attr(&self, input: &mut Vec<String>, context: &mut AttrContext) {
        context.domain = Some(Domain::Transfer);
        input.remove(0);
    }
}
