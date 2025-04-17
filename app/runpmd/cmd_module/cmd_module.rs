#[path = "flow/flow.rs"]
pub mod flow;

#[path = "port/port.rs"]
pub mod port;

// Command line parser is running in a dedicated thread.
// All ModuleOps objects must implement Send and Sync traits.
pub trait CmdModuleOps: Send + Sync {
    fn parse_cmd(&self, input: &mut Vec<String>);
}
