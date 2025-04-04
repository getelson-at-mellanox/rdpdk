use std::mem::offset_of;
use crate::dpdk_raw::rte_ethdev::{
    rte_flow_item_type_RTE_FLOW_ITEM_TYPE_IPV4,
    rte_ipv4_hdr};
use crate::cmdline::arg::Arg;
use crate::cmdline::arg::arg_int::ArgInt;
use crate::cmdline::arg::arg_net::Ipv4AddrArg;
use crate::cmdline::param::Param;
use crate::cmdline::flow::items::{FlowItems, Item};

pub(super) fn flow_item_create_ipv4(items_db: &mut FlowItems) {
    let src_arg: Box<dyn Arg> = Box::new(Ipv4AddrArg::new());
    let src_offset = offset_of!(rte_ipv4_hdr, src_addr);
    let src = Box::new(Param::from(
        "src",
        None,
        None,
        Some(vec![(src_arg, src_offset)]),
    ));

    let dst_arg: Box<dyn Arg> = Box::new(Ipv4AddrArg::new());
    let dst_offset= offset_of!(rte_ipv4_hdr, dst_addr);
    let dst = Box::new(Param::from(
        "dst",
        None,
        None,
        Some(vec![(dst_arg, dst_offset)]),
    ));

    let next_proto_arg: Box<dyn Arg> = Box::new(ArgInt::<u8>::new());
    let next_proto_offset = offset_of!(rte_ipv4_hdr, next_proto_id);
    let next_proto = Box::new(Param::from(
        "next_proto",
        None,
        None,
        Some(vec![(next_proto_arg, next_proto_offset)]),
    ));

    let ipv4 = Box::new(Param::from(
        "ipv4",
        Some(rte_flow_item_type_RTE_FLOW_ITEM_TYPE_IPV4 as isize),
        Some(size_of::<rte_ipv4_hdr>()),
        None,
    ));
    items_db.register(Item::from(ipv4, Some(vec![src, dst, next_proto])));
}