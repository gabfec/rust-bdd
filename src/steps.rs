use cucumber::{given, when, then, World};
use cucumber::gherkin::Step; // <-- Step contains the DocString
use crate::broker::Broker;
use serde_json::Value as JsonValue;
use anyhow::Result;

#[derive(World, Debug)]
pub struct MyWorld {
    pub broker: Option<Broker>,
    pub default_ip: String,
    pub sub_port: u16,
}

impl Default for MyWorld {
    fn default() -> Self {
        Self {
            broker: None,
            default_ip: "127.0.0.1".to_string(),
            sub_port: 4247,
        }
    }
}

#[given(regex = r"I run broker")]
async fn run_broker_default(world: &mut MyWorld) -> Result<()> {
    let ip = world.default_ip.clone();
    let broker = Broker::new()?;
    broker.connect(&ip)?;
    world.broker = Some(broker);
    Ok(())
}

#[given(regex = r"I run broker at (\S+)")]
async fn run_broker_at_ip(world: &mut MyWorld, ip: String) -> Result<()> {
    let broker = Broker::new()?;
    broker.connect(&ip)?;
    world.broker = Some(broker);
    Ok(())
}

#[when(regex = r"I send message (\w+)")]
async fn send_message(world: &mut MyWorld, name: String, step: &Step) -> Result<()> {
    let broker = world.broker.as_ref().expect("broker not started");

    let body: JsonValue = if let Some(ref doc) = step.docstring {
        serde_json::from_str(doc).expect("invalid JSON in DocString")
    } else {
        serde_json::json!({})
    };

    broker.send_message(&name, &body)?;
    Ok(())
}

#[then(regex = r"I expect message (\w+)")]
async fn expect_message(world: &mut MyWorld, name: String, step: &Step) -> Result<()> {
    let broker = world.broker.as_ref().expect("broker not started");

    let expected: JsonValue = if let Some(ref doc) = step.docstring {
        serde_json::from_str(doc).expect("invalid JSON in DocString")
    } else {
        serde_json::json!({})
    };

    let _got = broker.expect_message(&name, &expected, 5000)?;
    Ok(())
}
