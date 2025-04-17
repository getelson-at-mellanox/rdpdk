use std::mem::offset_of;
use rdpdk::dpdk_raw::ethdev_driver::rte_ether_hdr;
use rdpdk::dpdk_raw::rte_ethdev::{
    rte_flow_item_eth,
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_ETH
};
use rdpdk::cmdline::arg::Arg;
use rdpdk::cmdline::arg::arg_int::{ArgInt, ByteOrder};
use rdpdk::cmdline::arg::arg_net::EthAddrArg;
use rdpdk::cmdline::param::Param;
use crate::cmd_module::flow::items::{FlowItems, Item};


pub(super) fn flow_item_create_eth(items_db: &mut FlowItems) {
    let src_arg: Box<dyn Arg> = Box::new(EthAddrArg::new());
    let src_offset = offset_of!(rte_ether_hdr, src_addr);
    let src = Box::new(Param::from(
        "src",
        None,
        None,
        Some(vec![(src_arg, src_offset)]),
    ));

    let dst_arg: Box<dyn Arg> = Box::new(EthAddrArg::new());
    let dst_offset = offset_of!(rte_ether_hdr, dst_addr);
    let dst = Box::new(Param::from(
        "dst",
        None,
        None,
        Some(vec![(dst_arg, dst_offset)]),
    ));

    let type_arg: Box<dyn Arg> = Box::new(ArgInt::<u16>::new_with_order(ByteOrder::BigEndian));
    let type_offset = offset_of!(rte_ether_hdr, ether_type);
    let eth_type = Box::new(Param::from(
        "type",
        None,
        None,
        Some(vec![(type_arg, type_offset)]),
    ));

    let eth = Box::new(Param::from(
        "eth",
        Some(rte_flow_item_type_RTE_FLOW_ITEM_TYPE_ETH as isize),
        Some(size_of::<rte_flow_item_eth>()),
        None,
    ));

    items_db.register(Item::from(eth, Some(vec![src, dst, eth_type])));
}
