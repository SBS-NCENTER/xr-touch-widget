use std::io;
use std::net::{SocketAddr, UdpSocket};

use xrt_core::osc::{self, Incoming};

#[derive(Debug)]
pub enum Event {
    Trigger(String),
    PingAnswered(SocketAddr),
}

/// Stand-in for the UE box: logs triggers, answers pings with pongs.
/// Mirrors the exact interface contract the UE Blueprint must implement.
pub struct MockXr {
    socket: UdpSocket,
    reply_port: u16,
}

impl MockXr {
    pub fn bind(port: u16, reply_port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, reply_port })
    }

    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().expect("bound").port()
    }

    /// One receive step. Returns None when no packet is waiting.
    pub fn poll_once(&self) -> Option<Event> {
        let mut buf = [0u8; 1536];
        let (n, from) = self.socket.recv_from(&mut buf).ok()?;
        match osc::decode(&buf[..n]) {
            Incoming::Trigger(id) => Some(Event::Trigger(id)),
            Incoming::Ping => {
                let reply_to = SocketAddr::new(from.ip(), self.reply_port);
                let _ = self.socket.send_to(&osc::encode_pong(), reply_to);
                Some(Event::PingAnswered(reply_to))
            }
            _ => None,
        }
    }
}
