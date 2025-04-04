#[path = "arg/arg.rs"] // 2018 flat model
pub mod arg;

#[path = "param/param.rs"] // 2018 flat model
pub mod param;

#[path = "utils/flow/flow.rs"]
pub mod flow;

#[path = "utils/port/port.rs"]
pub mod port;

// command line parser is running in a desicated thread.
// All ModuleOps objects must implement Send and Sync traits.
pub trait ModuleOps: Send + Sync {
    fn parse_cmd(&self, input: &mut Vec<String>);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(4, 4);
    }
}
