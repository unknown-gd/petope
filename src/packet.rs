use bytes::{BufMut, BytesMut};
use etherparse::IpSlice;
use std::io::Write;

pub fn serialize_ip_slice(ip: IpSlice) -> BytesMut {
    let header = ip.to_header();
    let payload = ip.payload();
    let mut buf = BytesMut::with_capacity(header.header_len() + payload.payload.len()).writer();
    header.write(&mut buf).ok();
    buf.write(payload.payload).ok();
    buf.into_inner()
}
