use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};

use crate::config::{Target, ValueType};
use crate::osc::{self, Incoming};

#[derive(Debug)]
pub struct SendReport {
    pub ip: String,
    pub ok: bool,
    /// Populated with the io::Error's message when `ok` is false, so callers
    /// can log the actual send-failure reason (spec §8).
    pub error: Option<String>,
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
        // Parse the destination as an IP literal FIRST. A raw (&str, u16)
        // target otherwise triggers a BLOCKING DNS resolve for any non-literal
        // string (typo'd IP, hostname), which would freeze the single engine
        // thread (trigger + heartbeat) for seconds on-air. A bad IP becomes an
        // instant failed SendReport instead — the engine's !report.ok path
        // already logs + marks the link lost (red dot) rather than freezing.
        let addr = match ip.parse::<IpAddr>() {
            Ok(addr) => addr,
            Err(_) => {
                return SendReport {
                    ip: ip.to_string(),
                    ok: false,
                    error: Some("invalid IP: not an IP literal".into()),
                };
            }
        };
        match self.socket.send_to(bytes, SocketAddr::new(addr, port)) {
            Ok(_) => SendReport { ip: ip.to_string(), ok: true, error: None },
            Err(e) => SendReport { ip: ip.to_string(), ok: false, error: Some(e.to_string()) },
        }
    }

    /// Single shot, active targets only. Never retries (spec D2). Sends ONE
    /// OSC message — `address` + a single typed argument from (`value_type`,
    /// `value`) — built ONCE (D14). If the value can't be encoded (e.g. a bad
    /// int), every active target's SendReport is a failure carrying that error,
    /// so a bad value surfaces as a red dot on-air, never a panic.
    pub fn send_trigger(
        &self,
        address: &str,
        value_type: ValueType,
        value: &str,
        targets: &[Target],
        ue_port: u16,
    ) -> Vec<SendReport> {
        let bytes = match osc::encode_trigger(address, value_type, value) {
            Ok(bytes) => bytes,
            Err(e) => {
                return targets
                    .iter()
                    .filter(|t| t.active)
                    .map(|t| SendReport {
                        ip: t.ip.clone(),
                        ok: false,
                        error: Some(e.clone()),
                    })
                    .collect();
            }
        };
        targets
            .iter()
            .filter(|t| t.active)
            .map(|t| self.send_bytes(&bytes, &t.ip, ue_port))
            .collect()
    }

    /// Sends a ping to each target in the given list. Caller filters to active
    /// per D13 (the engine passes only active targets; this primitive does not
    /// filter — it sends to whatever list it is handed).
    pub fn send_ping(&self, targets: &[Target], ue_port: u16) -> Vec<SendReport> {
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
        let reports =
            osc_sock.send_trigger("/xrt/graphic", ValueType::String, "g1", &targets, ue_port);

        // only the active target got a send attempt
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].ip, "127.0.0.1");
        assert!(reports[0].ok);

        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut buf = [0u8; 1024];
        let (n, _) = ue.recv_from(&mut buf).unwrap();
        assert!(matches!(
            osc::decode(&buf[..n]),
            Incoming::Trigger { addr, arg } if addr == "/xrt/graphic" && arg.as_deref() == Some("g1")
        ));
        assert!(ue.recv_from(&mut buf).is_err(), "no second packet expected");
    }

    #[test]
    fn ping_goes_to_all_targets_and_pong_comes_back() {
        let (ue, ue_port) = fake_ue();
        let osc_sock = OscSocket::bind(0).unwrap();
        let listen_port = osc_sock.local_port();

        // The primitive ignores the active flag — it sends to whatever list it
        // is handed; the caller (engine) filters to active per D13. Passing an
        // inactive target here just exercises that no-filter contract.
        let targets = vec![target("127.0.0.1", false)];
        let reports = osc_sock.send_ping(&targets, ue_port);
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

    #[test]
    fn send_failure_is_reported_with_error_detail() {
        let osc_sock = OscSocket::bind(0).unwrap();
        // The socket is bound to an IPv4 address, so sending to an IPv6
        // literal fails synchronously with an address-family error — a
        // deterministic way to exercise the failure path without depending
        // on real network unreachability.
        let targets = vec![target("::1", true)];
        let reports = osc_sock.send_trigger("/xrt/graphic", ValueType::String, "g1", &targets, 9000);

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].ok);
        assert!(reports[0].error.is_some(), "failed send should carry an error detail");
    }

    #[test]
    fn non_ip_literal_target_fails_without_dns_resolve() {
        let osc_sock = OscSocket::bind(0).unwrap();
        // A hostname / typo'd IP is NOT an IP literal. It must be rejected up
        // front (instant failed SendReport) so the engine thread never enters a
        // blocking DNS resolve on-air. "localhost" would resolve if we let it
        // through; the point of this test is that we DON'T.
        let targets = vec![target("localhost", true)];
        let reports = osc_sock.send_trigger("/xrt/graphic", ValueType::String, "g1", &targets, 9000);

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].ok, "a non-IP-literal target must fail");
        assert_eq!(reports[0].ip, "localhost");
        assert_eq!(
            reports[0].error.as_deref(),
            Some("invalid IP: not an IP literal")
        );
    }

    #[test]
    fn bad_typed_value_fails_every_active_target_without_panic() {
        // D14: a value that can't be encoded for its type (int here) must
        // surface as a failed SendReport for EACH active target — never a
        // panic — so it shows as a red dot on-air, the same as a bad IP.
        let osc_sock = OscSocket::bind(0).unwrap();
        let targets = vec![
            target("127.0.0.1", true),
            target("127.0.0.2", true),
            target("127.0.0.3", false),
        ];
        let reports =
            osc_sock.send_trigger("/xrt/graphic", ValueType::Int, "notanint", &targets, 9000);

        // Only the two active targets, and both are failures carrying the error.
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().all(|r| !r.ok && r.error.is_some()));
    }
}
