mod l2net;
mod l3net;

use crate::cmdline::arg::{ArgData};
use crate::cmdline::param::Param;
use crate::dpdk_raw::rte_ethdev::{
    rte_flow_item, rte_flow_item_type,
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::addr_of;

pub trait ItemOps {
    fn name(&self) -> &str;
    fn parse_item(&self, input: &mut Vec<String>, context: &mut ItemsParserContext);
}

impl ItemOps for Param {
    fn name(&self) -> &str {
        &self.name
    }

    fn parse_item(&self, input: &mut Vec<String>, context: &mut ItemsParserContext) {
        if self.id.is_some() {
            context.id = self.id.unwrap() as rte_flow_item_type;
            context.size = self.size.unwrap();
            input.remove(0);
        } else if self.args.is_some() {
            let (arg_ops, arg_offset) = self.args.as_ref().unwrap().get(0).unwrap();
            let arg = arg_ops.serialize(&input[2]).unwrap();
            let src = &arg.data[0..arg.size];
            match input[1].as_str() {
                "is" => context.data.as_mut().unwrap().is_mod(src, *arg_offset),
                "spec" => context.data.as_mut().unwrap().spec_mod(src, *arg_offset),
                "mask" => context.data.as_mut().unwrap().mask_mod(src, *arg_offset),
                "last" => context.data.as_mut().unwrap().last_mod(src, *arg_offset),
                _ => {
                    panic!("Unknown item modifier '{}'", input[1]);
                }
            };

            for _ in 0..3 {
                input.remove(0);
            }
        }
    }
}

type ItemParamDB = HashMap<String, Box<dyn ItemOps>>;

pub struct Item {
    cmd: Box<dyn ItemOps>,
    param: Option<ItemParamDB>,
}

impl Item {
    pub fn from(cmd: Box<dyn ItemOps>, param: Option<Vec<Box<dyn ItemOps>>>) -> Self {
        Item {
            cmd: cmd,
            param: match param {
                None => None,
                Some(v) => {
                    let mut pdb = ItemParamDB::new();
                    for op in v {
                        pdb.insert(op.name().to_string(), op);
                    }
                    Some(pdb)
                }
            },
        }
    }

    pub fn parse(&self, input: &mut Vec<String>, context: &mut ItemsParserContext) {
        self.cmd.parse_item(input, context);
        match self.param {
            None => (),
            Some(ref pdb) => {
                if context.data.is_none() {
                    context.data = Some(ItemData::new());
                }
                loop {
                    match pdb.get(&input[0]) {
                        None => return,
                        Some(op) => {
                            op.parse_item(input, context);
                        }
                    }
                }
            }
        }
    }
}

type ItemsMap = HashMap<String, Item>;
pub struct FlowItems {
    map: RefCell<ItemsMap>,
    pub context: ItemsParserContext,
}

impl FlowItems {
    pub fn new() -> Self {
        let mut map = FlowItems {
            map: RefCell::new(ItemsMap::new()),
            context: ItemsParserContext::new(),
        };

        map.register(Item::from(ItemEnd::new(), None));
        map.register(Item::from(ItemSeparator::new(), None));

        l2net::flow_item_create_eth(&mut map);
        l3net::flow_item_create_ipv4(&mut map);
        map
    }

    pub fn register(&mut self, item: Item) {
        self.map
            .borrow_mut()
            .insert(item.cmd.name().to_string(), item);
    }

    pub fn parse_pattern(&mut self, input: &mut Vec<String>) {
        loop {
            match self.map.borrow().get(&input[0]) {
                None => return,
                Some(item) => {
                    item.parse(input, &mut self.context);
                }
            }
        }
    }

    pub fn get_raw_pattern(&self) -> *const rte_flow_item {
        self.context.raw.as_ptr() as *const rte_flow_item
    }
}

pub struct ItemData {
    pub spec: ArgData,
    pub mask: ArgData,
    pub last: ArgData,
}

impl ItemData {
    pub fn new() -> Self {
        ItemData {
            spec: ArgData::new(),
            mask: ArgData::new(),
            last: ArgData::new(),
        }
    }

    pub fn spec_mod(&mut self, src: &[u8], offset: usize) {
        self.spec.or_from_slice(src, offset).unwrap();
    }

    pub fn mask_mod(&mut self, src: &[u8], offset: usize) {
        self.mask.or_from_slice(src, offset).unwrap();
    }

    pub fn last_mod(&mut self, src: &[u8], offset: usize) {
        self.last.or_from_slice(src, offset).unwrap();
    }

    pub fn is_mod(&mut self, src: &[u8], offset: usize) {
        self.spec_mod(src, offset);
        self.mask_mod(&ArgData::DEFAULT_MASK[0..src.len()], offset);
    }
}

pub struct DpdkItem {
    pub itype: rte_flow_item_type,
    pub data: Option<ItemData>,
}

impl DpdkItem {
    pub fn from(t: rte_flow_item_type, data: Option<ItemData>) -> Self {
        DpdkItem {
            itype: t,
            data: data,
        }
    }
}

pub struct ItemsParserContext {
    id: rte_flow_item_type,
    size: usize,
    data: Option<ItemData>,
    items: Vec<DpdkItem>, // parsed items
    raw: Vec<rte_flow_item>,
}

impl ItemsParserContext {
    pub fn new() -> Self {
        ItemsParserContext {
            id: rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END,
            size: 0,
            data: None,
            items: Vec::new(),
            raw: Vec::new(),
        }
    }

    fn flush(&mut self) {
        self.id = rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END;
        self.data = None;
    }

    fn build_raw_pattern(&mut self) {
        for item in &self.items {
            let (spec, mask, last) = match &item.data {
                None => (
                    0 as *const ::std::os::raw::c_void,
                    0 as *const ::std::os::raw::c_void,
                    0 as *const ::std::os::raw::c_void,
                ),
                Some(data) => (
                    addr_of!(data.spec.data[0]) as *const ::std::os::raw::c_void,
                    addr_of!(data.mask.data[0]) as *const ::std::os::raw::c_void,
                    0 as *const ::std::os::raw::c_void,
                ),
            };
            let raw_item: rte_flow_item = rte_flow_item {
                type_: item.itype,
                spec: spec,
                mask: mask,
                last: last,
            };
            self.raw.push(raw_item);
        }
    }
}

struct ItemSeparator;

impl ItemSeparator {
    pub fn new() -> Box<dyn ItemOps> {
        Box::new(ItemSeparator)
    }
}

impl ItemOps for ItemSeparator {
    fn name(&self) -> &str {
        "/"
    }

    fn parse_item(&self, input: &mut Vec<String>, context: &mut ItemsParserContext) {
        context.items.push(DpdkItem::from(
            context.id as rte_flow_item_type,
            context.data.take(),
        ));
        context.flush();
        input.remove(0);
    }
}

struct ItemEnd;

impl ItemEnd {
    pub fn new() -> Box<dyn ItemOps> {
        Box::new(ItemEnd)
    }
}

impl ItemOps for ItemEnd {
    fn name(&self) -> &str {
        "end"
    }

    fn parse_item(&self, input: &mut Vec<String>, context: &mut ItemsParserContext) {
        context.items.push(DpdkItem::from(
            rte_flow_item_type_RTE_FLOW_ITEM_TYPE_END,
            None,
        ));
        context.build_raw_pattern();
        context.flush();
        input.remove(0);
    }
}

