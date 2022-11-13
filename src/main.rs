use std::time::Duration;

use clap::Parser;
use once_cell::sync::Lazy;
use rumqttc::{AsyncClient, QoS};
use sysinfo::{ComponentExt, System, SystemExt};
use tokio::time::sleep;

mod cli;
mod mqtt;

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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let matches = cli::Cli::parse();

    eprintln!("Broker: {}:{}", matches.broker, matches.port);
    eprintln!("Hostname: {}", HOSTNAME.as_str());

    let mut client = mqtt::connect(
        &matches.broker,
        matches.port,
        matches.username.as_deref(),
        matches.password.as_deref(),
        HOSTNAME.as_str(),
    )
    .await;
    eprintln!("MQTT {} initialized.", matches.broker);

    let mut sys = System::new_all();

    on_start(&mut client, &sys)
        .await
        .expect("publish on startup failed");

    loop {
        on_loop(&mut client, &mut sys)
            .await
            .expect("mqtt channel closed");
        sleep(Duration::from_secs(60)).await;
    }
}

async fn on_start(client: &mut AsyncClient, sys: &System) -> Result<(), rumqttc::ClientError> {
    async fn p<P: ToString + Send>(
        client: &mut AsyncClient,
        topic_part: &str,
        payload: P,
    ) -> Result<(), rumqttc::ClientError> {
        let topic = format!("{}/{topic_part}", HOSTNAME.as_str());
        let payload = payload.to_string();
        client.publish(topic, QOS, RETAIN, payload.trim()).await
    }

    p(client, "distribution", sys.distribution_id()).await?;

    if let Some(version) = sys.long_os_version() {
        p(client, "os-version", version).await?;
    }

    if let Some(kernel) = sys.kernel_version() {
        p(client, "kernel", kernel).await?;
    }

    p(client, "processors", sys.cpus().len()).await?;

    Ok(())
}

async fn on_loop(client: &mut AsyncClient, sys: &mut System) -> Result<(), rumqttc::ClientError> {
    async fn p<P: ToString + Send>(
        client: &mut AsyncClient,
        topic: String,
        payload: P,
    ) -> Result<(), rumqttc::ClientError> {
        let payload = payload.to_string();
        client.publish(topic, QOS, false, payload).await
    }

    static T_LOAD_1: Lazy<String> = Lazy::new(|| format!("{}/load/one", HOSTNAME.as_str()));
    static T_LOAD_5: Lazy<String> = Lazy::new(|| format!("{}/load/five", HOSTNAME.as_str()));
    static T_LOAD_15: Lazy<String> = Lazy::new(|| format!("{}/load/fifteen", HOSTNAME.as_str()));

    let load = sys.load_average();
    p(client, T_LOAD_1.to_string(), load.one).await?;
    p(client, T_LOAD_5.to_string(), load.five).await?;
    p(client, T_LOAD_15.to_string(), load.fifteen).await?;

    sys.refresh_components_list();
    for comp in sys.components() {
        let label = comp
            .label()
            .trim()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-");
        let topic = format!("{}/component-temperature/{label}", HOSTNAME.as_str());
        let temp = comp.temperature();
        p(client, topic, temp).await?;
    }

    Ok(())
}
