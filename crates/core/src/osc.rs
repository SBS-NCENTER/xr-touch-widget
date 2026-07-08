use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};

use crate::config::ValueType;

pub const ADDR_PING: &str = "/xrt/ping";
pub const ADDR_PONG: &str = "/xrt/pong";

/// A decoded inbound OSC message. The APP only consumes `Pong` (heartbeat);
/// mock-xr consumes `Ping` (replies pong) and logs `Trigger`. Since D14 the
/// app sends triggers to ARBITRARY addresses, so any message that is neither
/// ping nor pong is a `Trigger` carrying its address and a display string of
/// the first argument (if any). Non-message / garbage → `Other`.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    Ping,
    Pong,
    Trigger { addr: String, arg: Option<String> },
    Other,
}

fn encode_message(addr: &str, args: Vec<OscType>) -> Vec<u8> {
    // Encoding to an in-memory Vec never fails (the writer is infallible and
    // rosc does not validate the address string), so this is safe even for the
    // arbitrary, user-typed addresses D14 allows.
    encoder::encode(&OscPacket::Message(OscMessage {
        addr: addr.into(),
        args,
    }))
    .expect("OSC message encoding to a Vec is infallible")
}

/// Build the single OSC argument for a trigger from its typed value spec.
/// `None` → no argument at all; every other type parses `value` into the
/// matching OscType, returning a human-readable Err on a bad value so the
/// send path can surface it as a failed report instead of panicking on-air.
fn arg_for(value_type: ValueType, value: &str) -> Result<Option<OscType>, String> {
    match value_type {
        ValueType::None => Ok(None),
        ValueType::String => Ok(Some(OscType::String(value.into()))),
        ValueType::Int => value
            .trim()
            .parse::<i32>()
            .map(|n| Some(OscType::Int(n)))
            .map_err(|_| format!("invalid int: {value}")),
        ValueType::Float => value
            .trim()
            .parse::<f32>()
            .map(|f| Some(OscType::Float(f)))
            .map_err(|_| format!("invalid float: {value}")),
        ValueType::Bool => match value.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(Some(OscType::Bool(true))),
            "false" => Ok(Some(OscType::Bool(false))),
            _ => Err(format!("invalid bool: {value}")),
        },
    }
}

/// Encode ONE trigger message: `address` + a single typed argument built from
/// (`value_type`, `value`), or no argument when `value_type` is `None`.
/// Propagates the `arg_for` Err (bad value) so the caller never panics on-air.
pub fn encode_trigger(
    address: &str,
    value_type: ValueType,
    value: &str,
) -> Result<Vec<u8>, String> {
    let arg = arg_for(value_type, value)?;
    Ok(encode_message(address, arg.into_iter().collect()))
}

pub fn encode_ping() -> Vec<u8> {
    encode_message(ADDR_PING, vec![])
}

pub fn encode_pong() -> Vec<u8> {
    encode_message(ADDR_PONG, vec![])
}

/// Human-readable rendering of a trigger argument, for logging on the
/// receiving side (mock-xr). Covers the four types D14 can send; anything
/// else falls back to a debug rendering rather than being dropped.
fn display_arg(arg: &OscType) -> String {
    match arg {
        OscType::String(s) => s.clone(),
        OscType::Int(n) => n.to_string(),
        OscType::Float(f) => f.to_string(),
        OscType::Bool(b) => b.to_string(),
        other => format!("{other:?}"),
    }
}

pub fn decode(buf: &[u8]) -> Incoming {
    let Ok((_rest, packet)) = decoder::decode_udp(buf) else {
        return Incoming::Other;
    };
    let OscPacket::Message(msg) = packet else {
        return Incoming::Other;
    };
    match msg.addr.as_str() {
        ADDR_PING => Incoming::Ping,
        ADDR_PONG => Incoming::Pong,
        // Any other address is a trigger (D14: arbitrary addresses). Render
        // the first argument, if present, to a display string.
        _ => {
            let arg = msg.args.first().map(display_arg);
            Incoming::Trigger { addr: msg.addr, arg }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_roundtrips_with_address_and_value() {
        let bytes = encode_trigger("/xrt/graphic", ValueType::String, "lower_third_a").unwrap();
        match decode(&bytes) {
            Incoming::Trigger { addr, arg } => {
                assert_eq!(addr, "/xrt/graphic");
                assert_eq!(arg.as_deref(), Some("lower_third_a"));
            }
            other => panic!("expected Trigger, got {other:?}"),
        }
    }

    #[test]
    fn trigger_to_arbitrary_address_roundtrips() {
        // D14: the app may send to any address (not just /xrt/graphic).
        let bytes = encode_trigger("/scene/reset", ValueType::Int, "7").unwrap();
        match decode(&bytes) {
            Incoming::Trigger { addr, arg } => {
                assert_eq!(addr, "/scene/reset");
                assert_eq!(arg.as_deref(), Some("7"));
            }
            other => panic!("expected Trigger, got {other:?}"),
        }
    }

    #[test]
    fn trigger_with_no_arg_decodes_to_trigger_with_none() {
        let bytes = encode_trigger("/custom/addr", ValueType::None, "ignored").unwrap();
        match decode(&bytes) {
            Incoming::Trigger { addr, arg } => {
                assert_eq!(addr, "/custom/addr");
                assert_eq!(arg, None);
            }
            other => panic!("expected Trigger, got {other:?}"),
        }
    }

    #[test]
    fn ping_and_pong_roundtrip() {
        assert!(matches!(decode(&encode_ping()), Incoming::Ping));
        assert!(matches!(decode(&encode_pong()), Incoming::Pong));
    }

    #[test]
    fn garbage_bytes_are_other() {
        assert!(matches!(decode(&[0x01, 0x02, 0x03]), Incoming::Other));
    }

    #[test]
    fn arg_for_builds_each_typed_arg() {
        assert_eq!(arg_for(ValueType::None, "anything").unwrap(), None);
        assert_eq!(
            arg_for(ValueType::String, "hi").unwrap(),
            Some(OscType::String("hi".into()))
        );
        // Surrounding whitespace is trimmed for the numeric/bool types.
        assert_eq!(
            arg_for(ValueType::Int, " 42 ").unwrap(),
            Some(OscType::Int(42))
        );
        assert_eq!(
            arg_for(ValueType::Float, "3.5").unwrap(),
            Some(OscType::Float(3.5))
        );
        assert_eq!(
            arg_for(ValueType::Bool, "TRUE").unwrap(),
            Some(OscType::Bool(true))
        );
        assert_eq!(
            arg_for(ValueType::Bool, " false ").unwrap(),
            Some(OscType::Bool(false))
        );
    }

    #[test]
    fn arg_for_rejects_bad_values() {
        assert!(arg_for(ValueType::Int, "notanint").is_err());
        assert!(arg_for(ValueType::Float, "abc").is_err());
        assert!(arg_for(ValueType::Bool, "yes").is_err());
    }

    #[test]
    fn encode_trigger_errors_on_bad_value_without_panic() {
        let result = encode_trigger("/xrt/graphic", ValueType::Int, "nope");
        assert!(result.is_err());
    }
}
