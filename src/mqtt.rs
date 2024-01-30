use std::time::Duration;

use rumqttc::{AsyncClient, LastWill, MqttOptions};
use tokio::task;
use tokio::time::sleep;

use crate::{QOS, RETAIN};

pub async fn connect(
    broker: &str,
    port: std::num::NonZeroU16,
    username: Option<&str>,
    password: Option<&str>,
    hostname: &str,
) -> AsyncClient {
    let client_id = format!("mqtt-hostname-online-{hostname}");
    let mut mqttoptions = MqttOptions::new(client_id, broker, port.get());

    let t_status = format!("{hostname}/status");
    mqttoptions.set_last_will(LastWill::new(&t_status, "offline", QOS, RETAIN));

    if let Some(password) = password {
        let username = username.unwrap();
        mqttoptions.set_credentials(username, password);
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

    loop {
        let event = eventloop.poll().await.expect("MQTT connection");
        if let rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_)) = event {
            client
                .publish(&t_status, QOS, RETAIN, "online")
                .await
                .expect("Publish online status");
            break;
        }
    }

    let resultclient = client.clone();
    task::spawn(async move {
        loop {
            let event = eventloop.poll().await;
            match event {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                    client
                        .publish(&t_status, QOS, RETAIN, "online")
                        .await
                        .expect("Publish online status");
                }
                Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => {
                    eprintln!("MQTT Disconnect happening...");
                    break;
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("MQTT Connection Error: {err}");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    resultclient
}
