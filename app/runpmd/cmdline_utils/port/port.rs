use std::collections::HashMap;
use rdpdk::dpdk_raw::rte_ethdev::{rte_eth_promiscuous_disable, rte_eth_promiscuous_enable};
use rdpdk::cmdline::arg::*;
use rdpdk::cmdline::arg::arg_int::ArgInt;
use rdpdk::cmdline::{ModuleOps};

// port [set|show] [all | <port ID>] <command>
pub struct PortArg;

impl Arg for PortArg {
    fn serialize(&self, sample: &str) -> Result<ArgData, String> {
        if sample.eq("all") {
            Ok(ArgData::new_from_slice(&[0xffu8; ARG_DATA_SIZE]))
        } else {
            ArgInt::<u16>::new().serialize(sample)
        }
    }
}

enum PortType {
    None,
    AllPorts,
    SinglePort(u16),
}

enum PortCmdType {
    None,
    Set,
    Show,
}

struct PortParserContext {
    port_type: PortType,
    cmd_type: PortCmdType,
}

impl PortParserContext {
    pub fn new() -> Self {
        PortParserContext {
            port_type: PortType::None,
            cmd_type: PortCmdType::None,
        }
    }
}

trait PortOps {
    fn name(&self) -> &str;
    fn parse_port_cmd(&self, input: &mut Vec<String>, context: &mut PortParserContext);
}

type PortCmdMap = HashMap<String, Box<dyn PortOps>>;
pub struct PortModule {
    commands: PortCmdMap,
}

unsafe impl Send for PortModule {}
unsafe impl Sync for PortModule {}

impl ModuleOps for PortModule {

    fn parse_cmd(&self, input: &mut Vec<String>) {
        input.remove(0); // port

        let mut context = PortParserContext::new();

        context.cmd_type = match input[0].as_str() {
            "set" => PortCmdType::Set,
            "show" => PortCmdType::Show,
            _ => {
                return;
            }
        };

        context.port_type = match input[1].as_str() {
            "all" => PortType::AllPorts,
            _ => {
                let port = u16::from_str_radix(&input[1], 10).unwrap();
                PortType::SinglePort(port)
            }
        };

        input.remove(0);
        input.remove(0);

        loop {
            if input.len() == 0 { return; }
            match self.commands.get(&input[0]) {
                None => return,
                Some(op) => op.parse_port_cmd(input, &mut context),
            }
        }
    }
}

impl PortModule {
    pub fn new() -> Self {
        let mut map = PortModule {
            commands: PortCmdMap::new()
        };

        let promisc_cmd = PortCmdPromisc::new();
        map.commands.insert(promisc_cmd.name().to_string(), promisc_cmd);
        map
    }
}

struct PortCmdPromisc;
impl PortCmdPromisc {
    pub fn new() -> Box<dyn PortOps> {
        Box::new(PortCmdPromisc)
    }
}

impl PortOps for PortCmdPromisc {
    fn name(&self) -> &str {
        "promisc"
    }

    fn parse_port_cmd(&self, input: &mut Vec<String>, context: &mut PortParserContext) {

        let activate:bool = match input[1].as_str() {
            "on" | "1" => {
                true
            },
            "off" | "0" => {
                false
            },
            _ => {
                return;
            }
        };
        input.remove(0);
        input.remove(0);

        match context.cmd_type {
            PortCmdType::Set => {
                match context.port_type {
                    PortType::AllPorts => {},
                    PortType::SinglePort(port) => unsafe {
                        if activate {
                            rte_eth_promiscuous_enable(port);
                        } else {
                            rte_eth_promiscuous_disable(port);
                        }

                    },
                    _ => {return}
                }
            },
            PortCmdType::Show => {},
            _ => {return}
        }
    }
}

