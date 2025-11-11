use anyhow::{Result, Context};
use serde_json::Value as JsonValue;
use zmq::{Context as ZmqContext, Socket, PUB, SUB};
use crate::proto_dyn::ProtoDyn;
use std::fmt;
use prost_reflect::ReflectMessage;

pub struct Broker {
    //ctx: ZmqContext,
    pub_sock: Socket,
    sub_sock: Socket,
    proto: ProtoDyn,
}

impl fmt::Debug for Broker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Broker")
            .field("ctx", &"ZmqContext")
            .field("pub_sock", &"Socket(PUB)")
            .field("sub_sock", &"Socket(SUB)")
            .field("proto", &"ProtoDyn")
            .finish()
    }
}

impl Broker {
    pub fn new() -> Result<Self> {
        let ctx = ZmqContext::new();
        let pub_sock = ctx.socket(PUB).context("create pub")?;
        let sub_sock = ctx.socket(SUB).context("create sub")?;
        sub_sock.set_subscribe(b"").context("subscribe")?;
        let proto = ProtoDyn::new().context("proto")?;
        Ok(Self { pub_sock, sub_sock, proto })
    }

    /// Connects publisher to tcp://<ip>:4246 and subscriber to tcp://<ip>:4247 (matches your Python helper)
    pub fn connect(&self, ip: &str) -> Result<()> {
        self.pub_sock.connect(&format!(r"tcp://{}:4246", ip))?;
        self.sub_sock.connect(&format!(r"tcp://{}:4247", ip))?;
        std::thread::sleep(std::time::Duration::from_millis(200));
        Ok(())
    }

    /// Send protobuf message by name (message_name) with JSON body
    pub fn send_message(&self, message_name: &str, body: &JsonValue) -> Result<()> {
        let dm = self.proto.build_from_json(message_name, body)?;
        let payload = self.proto.encode_message(&dm)?;
        let topic = message_name.as_bytes();
        self.pub_sock.send_multipart(&[topic, &payload], 0).context("send multipart")?;
        Ok(())
    }


    /// Wait for a matching message and return JSON body when partial match found (timeout_ms in ms)
    pub fn expect_message(&self, message_name: &str, expected: &JsonValue, timeout_ms: i32) -> Result<JsonValue> {
        self.sub_sock.set_rcvtimeo(timeout_ms).context("set rcvtimeo")?;
        loop {
            let parts = match self.sub_sock.recv_multipart(0) {
                Ok(p) => p,
                Err(e) if e == zmq::Error::EAGAIN => anyhow::bail!(format!("timeout waiting for {}", message_name)),
                Err(e) => return Err(e).context("recv_multipart failed"),
            };
            if parts.len() != 2 { continue; }
            let topic = String::from_utf8_lossy(&parts[0]).to_string();
            let payload = &parts[1];
            // decode by topic name
            let msg_name = format!("company.project.v1.{}", topic);
            let dm = match self.proto.decode_message(msg_name.as_str(), payload) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let got_json = self.proto.to_json_value(&dm);
            if topic != message_name { continue; }
            //println!("Decoding topic '{}' with descriptor '{}'", topic, dm.descriptor().full_name());
            println!("Decoded: {:?}", dm);
            for f in dm.descriptor().fields() {
                println!(
                    "Field {}: {:?}",
                    f.name(),
                    dm.get_field(&f)
                );
            }

            // Convert expected enum strings to numbers for comparison
            let normalized_expected = self.normalize_json_for_comparison(expected, &dm)?;
            println!("Expected:{:?}", normalized_expected);
            println!("Received{:?}", got_json);
            if crate::proto_dyn::json_partial_match(&normalized_expected, &got_json) {
                return Ok(got_json);
            }
        }
    }

    /// Convert enum string values in expected JSON to their numeric equivalents
    fn normalize_json_for_comparison(&self, expected: &JsonValue, message: &prost_reflect::DynamicMessage) -> Result<JsonValue> {
        match expected {
            JsonValue::Object(map) => {
                let mut normalized = serde_json::Map::new();
                for (key, value) in map {
                    // Find the field descriptor for this key
                    if let Some(field_desc) = message.descriptor().fields().find(|f| f.name() == key) {
                        if field_desc.kind().as_enum().is_some() {
                            // This is an enum field, convert string to number
                            if let JsonValue::String(enum_name) = value {
                                if let Some(enum_desc) = field_desc.kind().as_enum() {
                                    // Find the enum value by name
                                    if let Some(enum_value) = enum_desc.values().find(|v| v.name() == enum_name) {
                                        normalized.insert(key.clone(), JsonValue::Number(serde_json::Number::from(enum_value.number())));
                                    } else {
                                        // Enum value not found, keep original
                                        normalized.insert(key.clone(), value.clone());
                                    }
                                } else {
                                    normalized.insert(key.clone(), value.clone());
                                }
                            } else {
                                normalized.insert(key.clone(), value.clone());
                            }
                        } else if field_desc.kind().as_message().is_some() {
                            // Recursively handle nested messages
                            // You might need to enhance this for nested enum handling
                            normalized.insert(key.clone(), value.clone());
                        } else {
                            normalized.insert(key.clone(), value.clone());
                        }
                    } else {
                        normalized.insert(key.clone(), value.clone());
                    }
                }
                Ok(JsonValue::Object(normalized))
            }
            _ => Ok(expected.clone())
        }
    }
}
