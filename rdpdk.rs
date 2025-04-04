#[path = "lib/cmdline/cmdline.rs"] // 2018 flat model
pub mod cmdline;

#[path = "lib/dpdk_raw/dpdk_raw.rs"] // 2018 flat model
pub mod dpdk_raw;

#[path = "lib/port.rs"] // 2018 flat model
pub mod port;

#[path = "lib/raw_port.rs"] // 2018 flat model
pub mod raw_port;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
