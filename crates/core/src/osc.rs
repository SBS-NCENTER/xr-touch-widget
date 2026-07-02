use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};

pub const ADDR_GRAPHIC: &str = "/xrt/graphic";
pub const ADDR_PING: &str = "/xrt/ping";
pub const ADDR_PONG: &str = "/xrt/pong";

#[derive(Debug, PartialEq)]
pub enum Incoming {
    Trigger(String),
    Ping,
    Pong,
    Other,
}

fn encode_message(addr: &str, args: Vec<OscType>) -> Vec<u8> {
    encoder::encode(&OscPacket::Message(OscMessage {
        addr: addr.into(),
        args,
    }))
    .expect("static OSC messages always encode")
}

pub fn encode_trigger(graphic_id: &str) -> Vec<u8> {
    encode_message(ADDR_GRAPHIC, vec![OscType::String(graphic_id.into())])
}

pub fn encode_ping() -> Vec<u8> {
    encode_message(ADDR_PING, vec![])
}

pub fn encode_pong() -> Vec<u8> {
    encode_message(ADDR_PONG, vec![])
}

pub fn decode(buf: &[u8]) -> Incoming {
    let Ok((_rest, packet)) = decoder::decode_udp(buf) else {
        return Incoming::Other;
    };
    let OscPacket::Message(msg) = packet else {
        return Incoming::Other;
    };
    match msg.addr.as_str() {
        ADDR_GRAPHIC => match msg.args.first() {
            Some(OscType::String(id)) => Incoming::Trigger(id.clone()),
            _ => Incoming::Other,
        },
        ADDR_PING => Incoming::Ping,
        ADDR_PONG => Incoming::Pong,
        _ => Incoming::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_roundtrips_with_graphic_id() {
        let bytes = encode_trigger("lower_third_a");
        assert!(matches!(
            decode(&bytes),
            Incoming::Trigger(id) if id == "lower_third_a"
        ));
    }

    #[test]
    fn ping_and_pong_roundtrip() {
        assert!(matches!(decode(&encode_ping()), Incoming::Ping));
        assert!(matches!(decode(&encode_pong()), Incoming::Pong));
    }

    #[test]
    fn unknown_address_is_other() {
        let msg = OscPacket::Message(OscMessage {
            addr: "/something/else".into(),
            args: vec![],
        });
        let bytes = encoder::encode(&msg).unwrap();
        assert!(matches!(decode(&bytes), Incoming::Other));
    }

    #[test]
    fn garbage_bytes_are_other() {
        assert!(matches!(decode(&[0x01, 0x02, 0x03]), Incoming::Other));
    }
}
