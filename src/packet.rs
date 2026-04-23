use bytes::{BufMut, BytesMut};
use etherparse::{
    Icmpv4Slice, Icmpv4Type, Icmpv6Slice, Icmpv6Type, IpNumber, IpSlice, PacketBuilder,
};
use std::io::Write;

pub fn serialize_ip_slice(ip: IpSlice) -> BytesMut {
    let header = ip.to_header();
    let payload = ip.payload();
    let mut buf = BytesMut::with_capacity(header.header_len() + payload.payload.len()).writer();
    header.write(&mut buf).ok();
    buf.write(payload.payload).ok();
    buf.into_inner()
}

pub fn echo_reply(ip: &IpSlice) -> Option<BytesMut> {
    if ip.payload_ip_number() != IpNumber::ICMP && ip.payload_ip_number() != IpNumber::IPV6_ICMP {
        return None;
    }

    match ip {
        IpSlice::Ipv4(v4) => match Icmpv4Slice::from_slice(v4.payload().payload) {
            Ok(s) => match s.icmp_type() {
                Icmpv4Type::EchoRequest(header) => {
                    let ipheader = v4.header();
                    let payload = s.payload();
                    let builder = PacketBuilder::ipv4(ipheader.destination(), ipheader.source(), 8)
                        .icmpv4_echo_reply(header.id, header.seq);

                    let mut buf = BytesMut::with_capacity(builder.size(payload.len())).writer();
                    builder.write(&mut buf, payload).ok();
                    Some(buf.into_inner())
                }
                _ => None,
            },
            Err(_) => None,
        },
        IpSlice::Ipv6(v6) => match Icmpv6Slice::from_slice(v6.payload().payload) {
            Ok(s) => match s.icmp_type() {
                Icmpv6Type::EchoRequest(header) => {
                    let ipheader = v6.header();
                    let payload = s.payload();
                    let builder = PacketBuilder::ipv6(ipheader.destination(), ipheader.source(), 8)
                        .icmpv6_echo_reply(header.id, header.seq);

                    let mut buf = BytesMut::with_capacity(builder.size(payload.len())).writer();
                    builder.write(&mut buf, payload).ok();
                    Some(buf.into_inner())
                }
                _ => None,
            },
            Err(_) => None,
        },
    }
}

// pub fn reply_to_request()
