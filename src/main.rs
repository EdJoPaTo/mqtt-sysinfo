use std::thread::sleep;
use std::time::Duration;

use clap::Parser;
use once_cell::sync::Lazy;
use rumqttc::{Client, LastWill, MqttOptions, QoS};
use sysinfo::{ComponentExt, SystemExt};

mod cli;

#[cfg(debug_assertions)]
const RETAIN: bool = false;
#[cfg(not(debug_assertions))]
const RETAIN: bool = true;

const QOS: QoS = QoS::AtLeastOnce;

static HOSTNAME: Lazy<String> = Lazy::new(|| {
    hostname::get()
        .expect("Failed to read hostname")
        .to_str()
        .expect("Failed to parse hostname to utf8")
        .to_string()
});
static T_STATUS: Lazy<String> = Lazy::new(|| format!("{}/status", HOSTNAME.as_str()));

fn main() {
    let (mut client, mut connection) = {
        let matches = cli::Cli::parse();

        eprintln!("Broker: {}:{}", matches.broker, matches.port);
        eprintln!("Status Topic: {}", T_STATUS.as_str());

        let client_id = format!("mqtt-hostname-online-{}", HOSTNAME.as_str());
        let mut mqttoptions = MqttOptions::new(client_id, matches.broker, matches.port);
        mqttoptions.set_last_will(LastWill::new(T_STATUS.as_str(), "offline", QOS, RETAIN));

        if let Some(password) = matches.password {
            let username = matches.username.unwrap();
            mqttoptions.set_credentials(username, password);
        }

        Client::new(mqttoptions, 25)
    };

    for notification in connection.iter() {
        match notification {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                client
                    .publish(T_STATUS.as_str(), QOS, RETAIN, "online")
                    .expect("mqtt channel closed");
                pubsys(&mut client).expect("mqtt channel closed");
                println!("connected and published");
            }
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::PingResp)) => {
                pubsys(&mut client).expect("mqtt channel closed");
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!("MQTT error: {err}");
                sleep(Duration::from_secs(5));
            }
        }
    }
}

fn pubsys(client: &mut Client) -> Result<(), rumqttc::ClientError> {
    static T_OS_VERSION: Lazy<String> = Lazy::new(|| format!("{}/os-version", HOSTNAME.as_str()));
    static T_PROCESSORS: Lazy<String> = Lazy::new(|| format!("{}/processors", HOSTNAME.as_str()));
    static T_LOAD_1: Lazy<String> = Lazy::new(|| format!("{}/load/one", HOSTNAME.as_str()));
    static T_LOAD_5: Lazy<String> = Lazy::new(|| format!("{}/load/five", HOSTNAME.as_str()));
    static T_LOAD_15: Lazy<String> = Lazy::new(|| format!("{}/load/fifteen", HOSTNAME.as_str()));

    let sys = sysinfo::System::new_all();
    if let Some(version) = sys.long_os_version() {
        client.publish(T_OS_VERSION.to_string(), QOS, RETAIN, version.trim())?;
    }

    client.publish(
        T_PROCESSORS.to_string(),
        QOS,
        RETAIN,
        sys.cpus().len().to_string(),
    )?;

    let load = sys.load_average();
    client.publish(T_LOAD_1.to_string(), QOS, false, load.one.to_string())?;
    client.publish(T_LOAD_5.to_string(), QOS, false, load.five.to_string())?;
    client.publish(T_LOAD_15.to_string(), QOS, false, load.fifteen.to_string())?;

    for comp in sys.components() {
        let label = comp
            .label()
            .trim()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-");
        let topic = format!("{}/component-temperature/{label}", HOSTNAME.as_str());
        let temp = comp.temperature();
        client.publish(topic, QOS, false, temp.to_string())?;
    }

    Ok(())
}
