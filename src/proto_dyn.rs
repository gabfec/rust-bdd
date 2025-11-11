
use anyhow::{anyhow, Result, Context};
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, ReflectMessage, Value as PbValue};
use prost_reflect::prost::Message as ProstMessage;
use prost_types::FileDescriptorSet;
use serde_json::Value as JsonValue;
use base64::Engine;
use base64::engine::general_purpose;

fn descriptor_pool() -> Result<DescriptorPool> {
    let bytes = include_bytes!("descriptor.bin");
    if bytes.is_empty() {
        anyhow::bail!("src/descriptor.bin is empty; generate it with protoc --descriptor_set_out=src/descriptor.bin --include_imports proto/PingPong.proto");
    }
    let descriptor_set = FileDescriptorSet::decode(&bytes[..])
        .context("failed to decode descriptor.bin as FileDescriptorSet")?;
    let pool = DescriptorPool::from_file_descriptor_set(descriptor_set)
        .context("failed to create descriptor pool from FileDescriptorSet")?;
    Ok(pool)
}

pub struct ProtoDyn {
    pool: DescriptorPool,
}

impl ProtoDyn {
    pub fn new() -> Result<Self> {
        Ok(Self { pool: descriptor_pool()? })
    }

    pub fn message_desc(&self, name: &str) -> Result<MessageDescriptor> {
        // Try both fully qualified and short name
        if let Some(m) = self.pool.get_message_by_name(name) {
            return Ok(m);
        }
        // Try searching by suffix (last segment)
        for m in self.pool.all_messages() {
            if let Some(n) = m.full_name().rsplit('.').next() {
                if n == name {
                    return Ok(m);
                }
            }
        }
        Err(anyhow!("message {} not found", name))
    }

    pub fn build_from_json(&self, name: &str, json: &JsonValue) -> Result<DynamicMessage> {
        let desc = self.message_desc(name)?;
        let mut msg = DynamicMessage::new(desc.clone());
        if let JsonValue::Object(map) = json {
            for (k, v) in map {
                if let Some(field) = desc.get_field_by_name(k) {
                    let val = json_to_pbvalue(&field.kind(), v, &self.pool)?;
                    msg.set_field(&field, val);
                } else {
                    return Err(anyhow!("unknown field {} for {}", k, name));
                }
            }
        }
        Ok(msg)
    }

    pub fn decode_message(&self, name: &str, bytes: &[u8]) -> Result<DynamicMessage> {
        let desc = self.message_desc(name)?;
        let mut msg = DynamicMessage::new(desc);
        msg.merge(bytes).context("merge failed")?;
        Ok(msg)
    }

    pub fn encode_message(&self, msg: &DynamicMessage) -> Result<Vec<u8>> {
        Ok(msg.encode_to_vec())
    }

    pub fn to_json_value(&self, msg: &DynamicMessage) -> JsonValue {
        dynamic_to_json(msg)
    }
}

fn json_to_pbvalue(kind: &prost_reflect::Kind, v: &JsonValue, pool: &DescriptorPool) -> Result<PbValue> {
    use prost_reflect::Kind;
    match kind {
        Kind::Bool => Ok(PbValue::Bool(v.as_bool().ok_or_else(|| anyhow!("expected bool"))?)),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => Ok(PbValue::I32(v.as_i64().ok_or_else(|| anyhow!("expected i32"))? as i32)),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => Ok(PbValue::I64(v.as_i64().ok_or_else(|| anyhow!("expected i64"))?)),
        Kind::Uint32 | Kind::Fixed32 => Ok(PbValue::U32(v.as_u64().ok_or_else(|| anyhow!("expected u32"))? as u32)),
        Kind::Uint64 | Kind::Fixed64 => Ok(PbValue::U64(v.as_u64().ok_or_else(|| anyhow!("expected u64"))?)),
        Kind::Float => Ok(PbValue::F32(v.as_f64().ok_or_else(|| anyhow!("expected f32"))? as f32)),
        Kind::Double => Ok(PbValue::F64(v.as_f64().ok_or_else(|| anyhow!("expected f64"))?)),
        Kind::String => Ok(PbValue::String(v.as_str().ok_or_else(|| anyhow!("expected string"))?.to_string())),
        Kind::Bytes => {
            let s = v.as_str().ok_or_else(|| anyhow!("expected base64 string for bytes"))?;
            let b = general_purpose::STANDARD.decode(s).context("bytes must be base64")?;
            Ok(PbValue::Bytes(b.into()))
        }
        Kind::Message(m) => {
            let mut dm = DynamicMessage::new(m.clone());
            let obj = v.as_object().ok_or_else(|| anyhow!("expected object"))?;
            for (k, vv) in obj.iter() {
                let f = dm.descriptor().get_field_by_name(k).ok_or_else(|| anyhow!("unknown field {}", k))?;
                let val = json_to_pbvalue(&f.kind(), vv, pool)?;
                dm.set_field(&f, val);
            }
            Ok(PbValue::Message(dm))
        }
        Kind::Enum(e) => {
            if let Some(s) = v.as_str() {
                let val = e.get_value_by_name(s).ok_or_else(|| anyhow!("unknown enum {}", s))?;
                Ok(PbValue::EnumNumber(val.number()))
            } else if let Some(i) = v.as_i64() {
                Ok(PbValue::EnumNumber(i as i32))
            } else {
                Err(anyhow!("enum must be string or int"))
            }
        }
    }
}

fn dynamic_to_json(msg: &DynamicMessage) -> JsonValue {
    let mut map = serde_json::Map::new();
    for f in msg.descriptor().fields() {
        if msg.has_field(&f) {
            let val = msg.get_field(&f);
            map.insert(f.name().to_string(), pbvalue_to_json(&val));
        }
    }
    JsonValue::Object(map)
}

fn pbvalue_to_json(v: &PbValue) -> JsonValue {
    use serde_json::json;
    match v {
        PbValue::Bool(b) => JsonValue::Bool(*b),
        PbValue::I32(i) => json!(*i),
        PbValue::I64(i) => json!(*i),
        PbValue::U32(i) => json!(*i),
        PbValue::U64(i) => json!(*i),
        PbValue::F32(f) => json!(*f),
        PbValue::F64(f) => json!(*f),
        PbValue::String(s) => JsonValue::String(s.clone()),
        PbValue::Bytes(b) => JsonValue::String(general_purpose::STANDARD.encode(b)),
        PbValue::Message(m) => dynamic_to_json(m),
        PbValue::EnumNumber(n) => json!(*n),
        PbValue::List(list) => JsonValue::Array(list.iter().map(pbvalue_to_json).collect()),
        PbValue::Map(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map.iter() {
                out.insert(format!("{:?}", k), pbvalue_to_json(v));
            }
            JsonValue::Object(out)
        }
    }
}

pub fn json_partial_match(expected: &JsonValue, actual: &JsonValue) -> bool {
    use serde_json::Value::*;
    match (expected, actual) {
        (Object(eo), Object(ao)) => eo.iter().all(|(k, ev)| ao.get(k).map(|av| json_partial_match(ev, av)).unwrap_or(false)),
        (Array(ea), Array(aa)) => {
            ea.iter().all(|ev| aa.iter().any(|av| json_partial_match(ev, av)))
        }
        _ => expected == actual,
    }
}
