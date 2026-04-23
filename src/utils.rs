use std::net::{Ipv4Addr, Ipv6Addr};

use base64::Engine;
use iroh::EndpointId;

pub fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub fn base64_decode(encoded: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(encoded)
}

pub fn ip_pair_from_id(id: EndpointId) -> (Ipv4Addr, Ipv6Addr) {
    let mut v4 = [0u8; 4];
    v4[0] = 10;
    v4[1..].copy_from_slice(&id[..3]);

    let mut v6 = [0u8; 16];
    v6[0] = 0xfd;
    v6[1..].copy_from_slice(&id[..15]);

    (Ipv4Addr::from_octets(v4), Ipv6Addr::from_octets(v6))
}
