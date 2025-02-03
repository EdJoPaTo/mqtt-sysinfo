use std::time::Duration;

use chrono::TimeZone;
use clap::Parser;
use once_cell::sync::Lazy;
use rumqttc::{AsyncClient, QoS};
use sysinfo::{Components, CpuRefreshKind, RefreshKind, System};
use tokio::time::sleep;

mod cli;
mod mqtt;

#[cfg(debug_assertions)]
const RETAIN: bool = false;
#[cfg(not(debug_assertions))]
const RETAIN: bool = true;

const QOS: QoS = QoS::AtLeastOnce;

static HOSTNAME: Lazy<String> =
    Lazy::new(|| System::host_name().expect("Hostname should be acquirable"));

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

    on_start(&client).await.expect("Initial publish");

    eprintln!("Initial MQTT publish done. Starting to publish live data now...");

    loop {
        on_loop(&client).await.expect("Regular update");
        sleep(Duration::from_secs(60)).await;
    }
}

async fn on_start(client: &AsyncClient) -> Result<(), rumqttc::ClientError> {
    #[allow(clippy::min_ident_chars)]
    async fn p<P: ToString + Send>(
        client: &AsyncClient,
        topic_part: &str,
        payload: P,
    ) -> Result<(), rumqttc::ClientError> {
        let topic = format!("{}/{topic_part}", HOSTNAME.as_str());
        let payload = payload.to_string();
        client.publish(topic, QOS, RETAIN, payload.trim()).await
    }

    {
        let topic_part = concat!(env!("CARGO_PKG_NAME"), "/repository");
        p(client, topic_part, env!("CARGO_PKG_REPOSITORY")).await?;

        let topic_part = concat!(env!("CARGO_PKG_NAME"), "/version");
        p(client, topic_part, env!("CARGO_PKG_VERSION")).await?;
    }

    if let Ok(boot_time) = System::boot_time().try_into() {
        let boot_time = chrono::Local.timestamp_opt(boot_time, 0).unwrap();
        p(client, "boot-time", boot_time.to_rfc3339()).await?;
    }

    p(client, "distribution", System::distribution_id()).await?;

    if let Some(name) = System::name() {
        p(client, "os-name", name).await?;
    }

    if let Some(version) = System::long_os_version() {
        p(client, "os-version", version).await?;
    }

    if let Some(kernel) = System::kernel_version() {
        p(client, "kernel", kernel).await?;
    }

    p(client, "cpu-arch", System::cpu_arch()).await?;

    {
        let sys =
            System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()));
        let cpus = sys.cpus();

        if let Some(cores) = sys.physical_core_count() {
            p(client, "cpu-cores", cores).await?;
        }
        p(client, "cpu-threads", cpus.len()).await?;

        let mut brands = cpus.iter().map(sysinfo::Cpu::brand).collect::<Vec<_>>();
        brands.sort_unstable();
        brands.dedup();
        p(client, "cpu-brand", brands.join("; ")).await?;

        let mut vendors = cpus.iter().map(sysinfo::Cpu::vendor_id).collect::<Vec<_>>();
        vendors.sort_unstable();
        vendors.dedup();
        p(client, "cpu-vendor", vendors.join("; ")).await?;
    }

    Ok(())
}

macro_rules! topic {
    ($topic:literal) => {{
        static TOPIC: Lazy<String> = Lazy::new(|| format!($topic, hostname = HOSTNAME.as_str()));
        TOPIC.to_string()
    }};
}

async fn on_loop(client: &AsyncClient) -> Result<(), rumqttc::ClientError> {
    #[allow(clippy::min_ident_chars)]
    async fn p<P: ToString + Send>(
        client: &AsyncClient,
        topic: String,
        payload: P,
    ) -> Result<(), rumqttc::ClientError> {
        let payload = payload.to_string();
        client.publish(topic, QOS, false, payload).await
    }

    let uptime = format_uptime(System::uptime());
    p(client, topic!("{hostname}/uptime"), uptime).await?;

    let load = System::load_average();
    p(client, topic!("{hostname}/load/one"), load.one).await?;
    p(client, topic!("{hostname}/load/five"), load.five).await?;
    p(client, topic!("{hostname}/load/fifteen"), load.fifteen).await?;

    for comp in Components::new_with_refreshed_list().list() {
        let Some(temp) = comp.temperature().filter(|temp| temp.is_finite()) else {
            continue;
        };
        let label = comp
            .label()
            .trim()
            .replace(|char: char| !char.is_ascii_alphanumeric(), "-");
        let topic = format!("{}/component-temperature/{label}", HOSTNAME.as_str());
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
