use std::io;
use std::net::{SocketAddr, UdpSocket};

use crate::config::Target;
use crate::osc::{self, Incoming};

#[derive(Debug)]
pub struct SendReport {
    pub ip: String,
    pub ok: bool,
}

/// One UDP socket, bound to listen_port, used for both sending and receiving.
/// Binding the send side means our pings carry listen_port as source port,
/// so receivers can reply to "source ip + listen_port" (interface contract).
pub struct OscSocket {
    socket: UdpSocket,
}

impl OscSocket {
    pub fn bind(listen_port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", listen_port))?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }

    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().expect("bound socket has addr").port()
    }

    fn send_bytes(&self, bytes: &[u8], ip: &str, port: u16) -> SendReport {
        let ok = self.socket.send_to(bytes, (ip, port)).is_ok();
        SendReport { ip: ip.to_string(), ok }
    }

    /// Single shot, active targets only. Never retries (spec D2).
    pub fn send_trigger(&self, graphic_id: &str, targets: &[Target], ue_port: u16) -> Vec<SendReport> {
        let bytes = osc::encode_trigger(graphic_id);
        targets
            .iter()
            .filter(|t| t.active)
            .map(|t| self.send_bytes(&bytes, &t.ip, ue_port))
            .collect()
    }

    /// Pings every registered target, active or not (status visible before switching).
    pub fn send_ping_all(&self, targets: &[Target], ue_port: u16) -> Vec<SendReport> {
        let bytes = osc::encode_ping();
        targets
            .iter()
            .map(|t| self.send_bytes(&bytes, &t.ip, ue_port))
            .collect()
    }

    pub fn try_recv(&self) -> Option<(Incoming, SocketAddr)> {
        let mut buf = [0u8; 1536];
        match self.socket.recv_from(&mut buf) {
            Ok((n, from)) => Some((osc::decode(&buf[..n]), from)),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bind a plain receiver on an ephemeral port, pretend it is the UE box.
    fn fake_ue() -> (UdpSocket, u16) {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = sock.local_addr().unwrap().port();
        (sock, port)
    }

    fn target(ip: &str, active: bool) -> Target {
        Target { name: "t".into(), ip: ip.into(), active }
    }

    #[test]
    fn trigger_reaches_only_active_targets() {
        let (ue, ue_port) = fake_ue();
        ue.set_nonblocking(true).unwrap();
        let osc_sock = OscSocket::bind(0).unwrap(); // 0 = ephemeral for tests

        let targets = vec![target("127.0.0.1", true), target("127.0.0.2", false)];
        let reports = osc_sock.send_trigger("g1", &targets, ue_port);

        // only the active target got a send attempt
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].ip, "127.0.0.1");
        assert!(reports[0].ok);

        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut buf = [0u8; 1024];
        let (n, _) = ue.recv_from(&mut buf).unwrap();
        assert!(matches!(osc::decode(&buf[..n]), Incoming::Trigger(id) if id == "g1"));
        assert!(ue.recv_from(&mut buf).is_err(), "no second packet expected");
    }

    #[test]
    fn ping_goes_to_all_targets_and_pong_comes_back() {
        let (ue, ue_port) = fake_ue();
        let osc_sock = OscSocket::bind(0).unwrap();
        let listen_port = osc_sock.local_port();

        let targets = vec![target("127.0.0.1", false)]; // inactive still pinged
        let reports = osc_sock.send_ping_all(&targets, ue_port);
        assert_eq!(reports.len(), 1);

        // fake UE receives ping, replies pong to source ip + listen_port
        let mut buf = [0u8; 1024];
        let (n, from) = ue.recv_from(&mut buf).unwrap();
        assert!(matches!(osc::decode(&buf[..n]), Incoming::Ping));
        let reply_to = SocketAddr::new(from.ip(), listen_port);
        ue.send_to(&osc::encode_pong(), reply_to).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
        let (incoming, from) = osc_sock.try_recv().expect("pong should be waiting");
        assert!(matches!(incoming, Incoming::Pong));
        assert_eq!(from.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn try_recv_returns_none_when_quiet() {
        let osc_sock = OscSocket::bind(0).unwrap();
        assert!(osc_sock.try_recv().is_none());
    }
}
