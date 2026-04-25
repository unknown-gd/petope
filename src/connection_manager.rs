use bytes::Bytes;
use dashmap::DashMap;
use iroh::{
    Endpoint, EndpointId,
    endpoint::{Connection, ConnectionError},
};
use ring_channel::RingSender;

use crate::utils;

pub const ALPN: &[u8] = b"petope/0";

pub struct ConnectionManager {
    endpoint: Endpoint,
    map: DashMap<EndpointId, Connection>,
    from_peer_tx: RingSender<Bytes>,
}

impl ConnectionManager {
    pub fn new(endpoint: Endpoint, from_peer_tx: RingSender<Bytes>) -> Self {
        ConnectionManager {
            map: DashMap::new(),
            endpoint,
            from_peer_tx,
        }
    }

    pub async fn run(&self) {
        // receive incoming connections and store them
        while let Some(incoming) = self.endpoint.accept().await {
            match incoming.await {
                Ok(conn) => {
                    self.set_connection(conn.clone());
                    Self::handle_connection(conn, self.from_peer_tx.clone()).await;
                }
                Err(e) => {
                    eprintln!("incoming connection failed: {:?}", e);
                }
            }
        }
    }

    // Retrieves existing connection for EndpointId, or tries to connect
    pub async fn get(&self, id: EndpointId) -> Option<Connection> {
        // first return existing connection if it exist
        if let Some(conn) = self.get_connection(&id) {
            return Some(conn);
        }

        let conn = match self.endpoint.connect(id, ALPN).await {
            Ok(conn) => Some(conn),
            Err(e) => {
                eprintln!("connect to peer {} failed: {:?}", utils::short_id(&id), e);
                None
            }
        };

        if let Some(conn) = &conn {
            self.set_connection(conn.clone());
            Self::handle_connection(conn.clone(), self.from_peer_tx.clone()).await;
        }

        conn
    }

    // replaces old connection with provided one, closes old if exist
    fn set_connection(&self, conn: Connection) {
        if let Some(old) = self.map.insert(conn.remote_id(), conn) {
            old.close(0u8.into(), b"outdated");
        }
    }

    fn get_connection(&self, id: &EndpointId) -> Option<Connection> {
        self.map.get(id).map(|v| v.clone())
    }

    async fn handle_connection(conn: Connection, from_peer_tx: RingSender<Bytes>) {
        tokio::spawn(async move {
            loop {
                match conn.read_datagram().await {
                    Ok(bytes) => {
                        // todo: filter bad ip packets
                        let _ = from_peer_tx.send(bytes);
                    }
                    Err(e) => {
                        match e {
                            ConnectionError::LocallyClosed => {}
                            e => eprintln!(
                                "connection error with {}: {:?}",
                                conn.remote_id().fmt_short(),
                                e
                            ),
                        };
                        break;
                    }
                }
            }
        });
    }
}
