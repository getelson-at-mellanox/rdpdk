
## Software Requirements

- rustc 1.85.0
- clang
- bindgen 0.71.1

## library structure

    Library structure:

    lib/
    ├── dpdk_raw/
    ├── cmdline/
    │   ├── param/
    │   └── arg/
    ├── port.rs
    └── raw_port.rs

- dpdk_raw: DPDK bindings library
- cmdline: interactive command line library
  - param: command line parameters API
  - arg: command line argumets API
  - utils: command line implementations
    - port: port configuration
    - flow: flow configuration
- port: port API
- raw_port: implement RawDpdkPort

### Environemnt Variables

- Manual raw-dpdk library binding:

  - DPDK_SOURCE_ROOT - DPDK sources root directory
  - DPDK_BUILD_ROOT  - DPDK build root directory
  - RDPDK_ROOT - RDPDK sources root directory

- PKG_CONFIG_PATH - should point to DPDK installation

### IO

Rdpdk defines port.rs::DpdkPortData trait for IO operations:

- Rx: fn rx_burst(&mut self, queue_id:u16, pkts: &\[*mut rte_mbuf]) -> Result<u16, String>;
- Tx: fn rx_burst(&mut self, queue_id:u16, pkts: &\[*mut rte_mbuf]) -> Result<u16, String>;

raw_port.rs::RawDpdkPort implements DpdkPortData for the general DPDK Rx/Tx through calls to
`rte_eth_fp_ops::rx_pkt_burst` and `rte_eth_fp_ops::tx_pkt_burst`

mlx5_port.rs::Mlx5Port implements DpdkPortData with dedicated MLX5 Rx/Tx functions.
  

## runpmd

runpmd is a testpmd like application for Rust.

### rustpmd supported commands:

#### general commands
- exit

#### port consiguration
- port set <port id> promisc [on|off] - enable / disable promiscuous port mode

#### flow commands

- Supported flow items:
  - eth: src, dst, proto
  - ipv4: src, dst, next_proto

- Supported actions:
  - drop

Example:
```
>>> port set 0 promisc on

# input command line can be split with the '\' character:

>>> flow create 0 ingress \
pattern eth src is aa:00:00:00:00:aa dst is b8:ce:f6:7b:d9:84 type is 0x800 / \
ipv4 src is 16.16.16.16 dst is 15.15.15.15 / end actions drop / end

>>> exit
```
