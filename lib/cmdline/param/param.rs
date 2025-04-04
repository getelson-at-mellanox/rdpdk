use crate::cmdline::arg::Arg;

pub struct Param {
    pub name: String,
    pub id: Option<isize>,
    pub size: Option<usize>,
    // TODO: replace vector with map
    pub args: Option<Vec<(Box<dyn Arg>, usize)>>,
}

pub type ParamArg = (Box<dyn Arg>, usize);
impl Param {
    pub fn from(
        name: &str,
        id: Option<isize>,
        size: Option<usize>,
        args: Option<Vec<ParamArg>>,
    ) -> Param {
        Param {
            name: name.to_string(),
            id: id,
            size: size,
            args: args,
        }
    }
}
