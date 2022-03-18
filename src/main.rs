use std::thread::sleep;
use std::time::Duration;

use rumqttc::{Client, LastWill, MqttOptions, QoS};

mod cli;

#[cfg(debug_assertions)]
const RETAIN: bool = false;
#[cfg(not(debug_assertions))]
const RETAIN: bool = true;

const QOS: QoS = QoS::AtLeastOnce;

fn main() {
    let hostname = hostname::get().expect("Failed to read hostname");
    let hostname = hostname.to_str().expect("Failed to parse hostname to utf8");
    let status_topic = format!("{}/status", hostname);
    println!("Status Topic: {}", status_topic);

    let (mut client, mut connection) = {
        let matches = cli::build().get_matches();
        let host = matches.value_of("broker").unwrap();
        let port = matches
            .value_of("port")
            .and_then(|s| s.parse().ok())
            .unwrap();

        let client_id = format!("mqtt-hostname-online-{}", hostname);
        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_last_will(LastWill::new(&status_topic, "offline", QOS, RETAIN));

        if let Some(password) = matches.value_of("password") {
            let username = matches.value_of("username").unwrap();
            mqttoptions.set_credentials(username, password);
        }

        Client::new(mqttoptions, 10)
    };

    for notification in connection.iter() {
        match notification {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                client
                    .publish(&status_topic, QOS, RETAIN, "online")
                    .expect("mqtt channel closed");
                println!("connected and published");
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!("MQTT error: {}", err);
                sleep(Duration::from_secs(5));
            }
        }
    }
}
