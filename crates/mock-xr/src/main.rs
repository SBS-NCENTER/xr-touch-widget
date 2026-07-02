use mock_xr::{Event, MockXr};

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(8000);
    let reply_port: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(8001);

    let mock = MockXr::bind(port, reply_port).expect("failed to bind UDP port");
    println!("mock-xr listening on :{port}, answering pongs to <sender-ip>:{reply_port}");

    loop {
        match mock.poll_once() {
            Some(Event::Trigger(id)) => println!("TRIGGER  graphic_id={id}"),
            Some(Event::PingAnswered(to)) => println!("PING     -> pong to {to}"),
            None => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
}
