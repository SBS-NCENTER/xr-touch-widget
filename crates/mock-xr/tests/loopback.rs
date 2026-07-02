use mock_xr::{Event, MockXr};
use xrt_core::config::Target;
use xrt_core::heartbeat::{Heartbeat, LinkStatus};
use xrt_core::net::OscSocket;
use xrt_core::osc::Incoming;

fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

#[test]
fn full_loop_trigger_ping_pong_and_heartbeat() {
    // widget side: ephemeral listen port
    let widget = OscSocket::bind(0).unwrap();
    let listen_port = widget.local_port();

    // UE side: mock on ephemeral port, replying pongs to our listen port
    let mock = MockXr::bind(0, listen_port).unwrap();
    let ue_port = mock.local_port();

    let targets = vec![Target {
        name: "MOCK".into(),
        ip: "127.0.0.1".into(),
        active: true,
    }];

    // --- trigger path ---
    widget.send_trigger("stinger_open", &targets, ue_port);
    wait_ms(50);
    assert!(matches!(
        mock.poll_once(),
        Some(Event::Trigger(id)) if id == "stinger_open"
    ));

    // --- heartbeat path ---
    let mut hb = Heartbeat::new(3);
    let ips: Vec<String> = targets.iter().map(|t| t.ip.clone()).collect();

    widget.send_ping_all(&targets, ue_port);
    hb.on_tick(&ips);
    wait_ms(50);
    assert!(matches!(mock.poll_once(), Some(Event::PingAnswered(_))));
    wait_ms(50);

    if let Some((Incoming::Pong, from)) = widget.try_recv() {
        hb.on_pong(&from.ip().to_string());
    }
    assert_eq!(hb.status("127.0.0.1"), LinkStatus::Connected);
}
