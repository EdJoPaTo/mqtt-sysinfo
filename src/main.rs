use std::time::Duration;

use chrono::TimeZone;
use clap::Parser;
use once_cell::sync::Lazy;
use rumqttc::{AsyncClient, QoS};
use sysinfo::{ComponentExt, CpuExt, System, SystemExt};
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
        .expect("Hostname should be acquirable")
        .to_str()
        .expect("Hostname should be UTF-8")
        .to_string()
});

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let matches = cli::Cli::parse();

    eprintln!("Hostname: {}", HOSTNAME.as_str());
    eprintln!("MQTT Broker: {}:{}", matches.broker, matches.port);

    let client = mqtt::connect(
        &matches.broker,
        matches.port,
        matches.username.as_deref(),
        matches.password.as_deref(),
        HOSTNAME.as_str(),
    )
    .await;

    let mut sys = System::new_all();

    on_start(&client, &sys).await.expect("Initial publish");

    eprintln!("Initial MQTT publish done. Starting to publish live data now...");

    loop {
        on_loop(&client, &mut sys).await.expect("Regular update");
        sleep(Duration::from_secs(60)).await;
    }
}

async fn on_start(client: &AsyncClient, sys: &System) -> Result<(), rumqttc::ClientError> {
    async fn p<P: ToString + Send>(
        client: &AsyncClient,
        topic_part: &str,
        payload: P,
    ) -> Result<(), rumqttc::ClientError> {
        let topic = format!("{}/{topic_part}", HOSTNAME.as_str());
        let payload = payload.to_string();
        client.publish(topic, QOS, RETAIN, payload.trim()).await
    }

    if let Ok(boot_time) = sys.boot_time().try_into() {
        let boot_time = chrono::Local.timestamp_opt(boot_time, 0).unwrap();
        p(client, "boot-time", boot_time.to_rfc3339()).await?;
    }

    p(client, "distribution", sys.distribution_id()).await?;

    if let Some(version) = sys.long_os_version() {
        p(client, "os-version", version).await?;
    }

    if let Some(kernel) = sys.kernel_version() {
        p(client, "kernel", kernel).await?;
    }

    if let Some(cores) = sys.physical_core_count() {
        p(client, "cpu-cores", cores).await?;
    }
    p(client, "cpu-threads", sys.cpus().len()).await?;

    let cpu = sys.global_cpu_info();
    p(client, "cpu-vendor", cpu.vendor_id()).await?;
    p(client, "cpu-brand", cpu.brand()).await?;

    Ok(())
}

async fn on_loop(client: &AsyncClient, sys: &mut System) -> Result<(), rumqttc::ClientError> {
    async fn p<P: ToString + Send>(
        client: &AsyncClient,
        topic: String,
        payload: P,
    ) -> Result<(), rumqttc::ClientError> {
        let payload = payload.to_string();
        client.publish(topic, QOS, false, payload).await
    }

    static T_UPTIME: Lazy<String> = Lazy::new(|| format!("{}/uptime", HOSTNAME.as_str()));
    static T_LOAD_1: Lazy<String> = Lazy::new(|| format!("{}/load/one", HOSTNAME.as_str()));
    static T_LOAD_5: Lazy<String> = Lazy::new(|| format!("{}/load/five", HOSTNAME.as_str()));
    static T_LOAD_15: Lazy<String> = Lazy::new(|| format!("{}/load/fifteen", HOSTNAME.as_str()));

    p(client, T_UPTIME.to_string(), format_uptime(sys.uptime())).await?;

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

fn format_uptime(uptime: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let seconds = uptime as f64;
    let minutes = seconds / 60.0;
    if minutes < 100.0 {
        return format!("{minutes:.1} minutes");
    }
    let hours = minutes / 60.0;
    if hours < 24.0 {
        return format!("{hours:.1} hours");
    }
    let days = hours / 24.0;
    format!("{days:.1} days")
}

#[test]
fn format_days_examples() {
    assert_eq!(format_uptime(30), "0.5 minutes");
    assert_eq!(format_uptime(60 * 5), "5.0 minutes");
    assert_eq!(format_uptime(60 * 90), "90.0 minutes");
    assert_eq!(format_uptime(60 * 60 * 5), "5.0 hours");
    assert_eq!(format_uptime(60 * 60 * 20), "20.0 hours");
    assert_eq!(format_uptime(60 * 60 * 24 * 5), "5.0 days");
}
