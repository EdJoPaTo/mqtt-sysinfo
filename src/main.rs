use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

use rumqttc::{Client, ConnectionError, Event, LastWill, MqttOptions, QoS};

mod cli;

#[cfg(debug_assertions)]
const RETAIN: bool = false;
#[cfg(not(debug_assertions))]
const RETAIN: bool = true;

fn main() {
    let hostname = hostname::get().expect("Failed to read hostname");
    let hostname = hostname.to_str().expect("Failed to parse hostname to utf8");
    let topic = format!("{}/connected", hostname);

    let (mut client, mut connection) = {
        let matches = cli::build().get_matches();
        let host = matches
            .value_of("Host")
            .expect("Host could not be read from command line");
        let port = matches
            .value_of("Port")
            .and_then(|s| s.parse::<u16>().ok())
            .expect("MQTT Server Port could not be read from command line");

        let client_id = format!("mqtt-hostname-online-{}", hostname);
        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_last_will(LastWill::new(&topic, "0", QoS::AtLeastOnce, RETAIN));

        Client::new(mqttoptions, 10)
    };

    for notification in connection.iter() {
        if let Err(err) = handle_notification(&mut client, &topic, notification) {
            eprintln!("MQTT error: {}", err);
            sleep(Duration::from_secs(5));
        }
    }
}

fn handle_notification(
    client: &mut Client,
    topic: &str,
    notification: Result<Event, ConnectionError>,
) -> Result<(), Box<dyn Error>> {
    if let rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_)) = notification? {
        client.publish(topic, QoS::AtLeastOnce, RETAIN, "2")?;
        println!("connected and published {} 2", topic);
    }

    Ok(())
}
