use std::net::{Ipv4Addr, Ipv6Addr};

use base64::Engine;
use iroh::EndpointId;

pub fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub fn base64_decode(encoded: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(encoded)
}

pub fn ipv4_from_id(id: &EndpointId) -> Ipv4Addr {
    Ipv4Addr::new(10, id[0], id[1], id[2])
}

pub fn ipv6_from_id(id: &EndpointId) -> Ipv6Addr {
    Ipv6Addr::new(0xfd22, id[0] as u16, 0, 0, 0, 0, 0, 1)
}
