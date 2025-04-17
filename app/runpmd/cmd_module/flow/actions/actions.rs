use rdpdk::cmdline::arg::arg_int::ArgInt;
use rdpdk::cmdline::arg::{Arg, ArgData};
use rdpdk::cmdline::param::Param;
use rdpdk::dpdk_raw::rte_ethdev::{
    rte_flow_action, rte_flow_action_ethdev, rte_flow_action_type,
    rte_flow_action_type_RTE_FLOW_ACTION_TYPE_DROP, rte_flow_action_type_RTE_FLOW_ACTION_TYPE_END,
    rte_flow_action_type_RTE_FLOW_ACTION_TYPE_REPRESENTED_PORT,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::offset_of;
use std::ptr::{addr_of, null};

struct DpdkAction {
    id: rte_flow_action_type,
    data: Option<ArgData>,
}
impl DpdkAction {
    fn from(id: rte_flow_action_type, data: Option<ArgData>) -> Self {
        DpdkAction { id, data }
    }
}

pub struct ActionsParserContext {
    id: rte_flow_action_type,
    data: Option<ArgData>,
    actions: Vec<DpdkAction>,
    raw_actions: Vec<rte_flow_action>,
}

impl ActionsParserContext {
    fn new() -> Self {
        ActionsParserContext {
            id: rte_flow_action_type_RTE_FLOW_ACTION_TYPE_END,
            data: None,
            actions: Vec::new(),
            raw_actions: Vec::new(),
        }
    }

    pub fn build_raw_actions(&mut self) {
        for action in &self.actions {
            let raw_action: rte_flow_action = rte_flow_action {
                type_: action.id,
                conf: match &action.data {
                    None => null(),
                    Some(data) => addr_of!(data.data[0]) as *const ::std::os::raw::c_void,
                },
            };
            self.raw_actions.push(raw_action);
        }
    }
}

pub trait ActionOps {
    fn name(&self) -> &str;

    fn parse_action(&self, input: &mut Vec<String>, context: &mut ActionsParserContext);
}

type ParamMap = HashMap<String, Box<dyn ActionOps>>;

pub struct Action {
    pub cmd: Box<dyn ActionOps>,
    pub param: Option<ParamMap>,
}

impl Action {
    pub fn from(cmd: Box<dyn ActionOps>, param: Option<Vec<Box<dyn ActionOps>>>) -> Self {
        Action {
            cmd: cmd,
            param: match param {
                None => None,
                Some(v) => {
                    let mut pdb = ParamMap::new();

                    for op in v {
                        pdb.insert(op.name().to_string(), op);
                    }
                    Some(pdb)
                }
            },
        }
    }

    fn parse(&self, input: &mut Vec<String>, context: &mut ActionsParserContext) {
        self.cmd.parse_action(input, context);
        match self.param {
            None => (),
            Some(ref pmap) => {
                if context.data.is_none() {
                    context.data = Some(ArgData::new())
                }
                loop {
                    match pmap.get(&input[0]) {
                        None => return,
                        Some(op) => {
                            op.parse_action(input, context);
                        }
                    }
                }
            }
        }
    }
}

type ActionsMap = HashMap<String, Action>;

pub struct FlowActions {
    map: RefCell<ActionsMap>,
    context: ActionsParserContext,
}

impl FlowActions {
    pub fn new() -> FlowActions {
        let mut adb = FlowActions {
            map: RefCell::new(HashMap::new()),
            context: ActionsParserContext::new(),
        };

        adb.register(Action::from(ActionEnd::new(), None));
        adb.register(Action::from(ActionDrop::new(), None));
        adb.register(Action::from(ActionSeparator::new(), None));

        adb
    }

    pub fn register(&mut self, action: Action) {
        self.map
            .borrow_mut()
            .insert(action.cmd.name().to_string(), action);
    }

    pub fn parse_actions(&mut self, input: &mut Vec<String>) {
        loop {
            match self.map.borrow().get(&input[0]) {
                None => return,
                Some(action) => {
                    action.parse(input, &mut self.context);
                }
            }
            if input.len() == 0 {
                return;
            }
        }
    }

    pub fn get_raw_actions(&self) -> *const rte_flow_action {
        self.context.raw_actions.as_ptr() as *const rte_flow_action
    }
}

impl ActionOps for Param {
    fn name(&self) -> &str {
        &self.name
    }

    // Default flow actions parser
    fn parse_action(&self, input: &mut Vec<String>, context: &mut ActionsParserContext) {
        if self.id.is_some() {
            context.id = self.id.unwrap() as rte_flow_action_type;
            input.remove(0);
        } else if self.args.is_some() {
            let (arg_op, offset) = self.args.as_ref().unwrap().get(0).unwrap();
            let arg = arg_op.serialize(&input[1]).unwrap();
            let slice = &arg.data[0..arg.size];
            context
                .data
                .as_mut()
                .unwrap()
                .or_from_slice(slice, *offset)
                .unwrap();

            input.remove(0);
            input.remove(0);
        }
    }
}

struct ActionSeparator;
impl ActionSeparator {
    fn new() -> Box<dyn ActionOps> {
        Box::new(ActionSeparator)
    }
}

impl ActionOps for ActionSeparator {
    fn name(&self) -> &str {
        "/"
    }

    fn parse_action(&self, input: &mut Vec<String>, _context: &mut ActionsParserContext) {
        input.remove(0);
    }
}

struct ActionEnd;
impl ActionEnd {
    fn new() -> Box<dyn ActionOps> {
        Box::new(ActionEnd)
    }
}

impl ActionOps for ActionEnd {
    fn name(&self) -> &str {
        "end"
    }

    fn parse_action(&self, input: &mut Vec<String>, context: &mut ActionsParserContext) {
        context.actions.push(DpdkAction::from(
            rte_flow_action_type_RTE_FLOW_ACTION_TYPE_END,
            None,
        ));
        context.build_raw_actions();
        input.remove(0);
    }
}

struct ActionDrop;
impl ActionDrop {
    fn new() -> Box<dyn ActionOps> {
        Box::new(ActionDrop)
    }
}

impl ActionOps for ActionDrop {
    fn name(&self) -> &str {
        "drop"
    }
    fn parse_action(&self, input: &mut Vec<String>, context: &mut ActionsParserContext) {
        context.actions.push(DpdkAction::from(
            rte_flow_action_type_RTE_FLOW_ACTION_TYPE_DROP,
            None,
        ));
        input.remove(0);
    }
}

pub fn flow_action_create_represented_port(actions_db: &mut FlowActions) {
    /*
     *   / represented_port ethdev_port_id <port> /
     */
    let port_arg: Box<dyn Arg> = Box::new(ArgInt::<u16>::new());
    let port_offset = offset_of!(rte_flow_action_ethdev, port_id);
    let port_param = Box::new(Param::from(
        "ethdev_port_id",
        None,
        None,
        Some(vec![(port_arg, port_offset)]),
    ));

    let port = Box::new(Param::from(
        "represented_port",
        Some(rte_flow_action_type_RTE_FLOW_ACTION_TYPE_REPRESENTED_PORT as isize),
        Some(size_of::<rte_flow_action_ethdev>()),
        None,
    ));

    actions_db.register(Action::from(port, Some(vec![port_param])))
}
