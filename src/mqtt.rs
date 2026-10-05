use std::time::{Duration, Instant};

use rumqttc::{AsyncClient, LastWill, MqttOptions};
use tokio::task;
use tokio::time::sleep;

use crate::{INTERVAL, QOS, RETAIN};

const KEEP_ALIVE: Duration = Duration::from_secs(INTERVAL.as_secs() * 3 / 2);

pub async fn connect(
    broker: &str,
    port: std::num::NonZeroU16,
    username: Option<&str>,
    password: Option<&str>,
    hostname: &str,
) -> AsyncClient {
    let client_id = format!("mqtt-sysinfo-{hostname}");
    let mut mqttoptions = MqttOptions::new(client_id, broker, port.get());
    mqttoptions.set_keep_alive(KEEP_ALIVE);

    let t_status = format!("{hostname}/status");
    mqttoptions.set_last_will(LastWill::new(&t_status, "offline", QOS, RETAIN));

    if let Some(password) = password {
        let username = username.unwrap();
        mqttoptions.set_credentials(username, password);
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

    loop {
        match eventloop.poll().await {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(packet))) => {
                eprintln!("Initial MQTT connection successful {packet:?}");
                break;
            }
            Ok(event) => eprintln!("Unexpected event on expected initial ConnAck: {event:?}"),
            Err(err) => panic!("Initial MQTT connection error: {err}"),
        }
    }
    let mut last_incoming = Instant::now();
    on_connect(client.clone(), t_status.clone());

    let resultclient = client.clone();
    task::spawn(async move {
        loop {
            let event = eventloop.poll().await;
            match event {
                Ok(rumqttc::Event::Incoming(event)) => {
                    last_incoming = Instant::now();
                    if let rumqttc::Packet::ConnAck(packet) = event {
                        eprintln!("MQTT connected {packet:?}");
                        on_connect(client.clone(), t_status.clone());
                    }
                }
                Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => {
                    eprintln!("MQTT Disconnect happening...");
                    break;
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("MQTT Connection Error: {err}");
                    assert!(
                        last_incoming.elapsed() < KEEP_ALIVE,
                        "no incoming for more than {KEEP_ALIVE:?}"
                    );
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    resultclient
}

fn on_connect(client: AsyncClient, topic: String) {
    task::spawn(async move {
        client
            .publish(topic, QOS, RETAIN, "online")
            .await
            .expect("MQTT should publish connection status");
        eprintln!("MQTT connection fully initialized");
    });
}
