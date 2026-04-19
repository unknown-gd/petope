use std::{io, net::SocketAddr, process::Command};
use tokio::net::TcpSocket;

pub fn reusable_socket(bind: Option<SocketAddr>) -> io::Result<TcpSocket> {
    let socket = TcpSocket::new_v4()?;

    socket.set_reuseport(true)?;

    if let Some(bind) = bind {
        socket.bind(bind)?;
    }

    Ok(socket)
}

pub fn get_hostname() -> io::Result<String> {
    let output = Command::new("hostname").output()?.stdout;
    let mut result = String::from(std::str::from_utf8(&output).unwrap().trim());

    if !result.ends_with(".local") {
        result += ".local"
    }

    if !result.ends_with('.') {
        result += "."
    }

    Ok(result)
}
