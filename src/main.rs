use std::sync::LazyLock;
use std::time::Duration;

use chrono::TimeZone;
use clap::Parser;
use rumqttc::{AsyncClient, QoS};
use sysinfo::{Components, CpuRefreshKind, Motherboard, Product, RefreshKind, System};
use tokio::time::sleep;

mod cli;
mod mqtt;

const RETAIN: bool = cfg!(not(debug_assertions));

const QOS: QoS = QoS::AtLeastOnce;

static HOSTNAME: LazyLock<String> =
    LazyLock::new(|| System::host_name().expect("Hostname should be acquirable"));

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

    /// publish optional
    async fn po<P: ToString + Send>(
        client: &AsyncClient,
        topic_part: &str,
        payload: Option<P>,
    ) -> Result<(), rumqttc::ClientError> {
        if let Some(payload) = payload {
            p(client, topic_part, payload).await?;
        }
        Ok(())
    }

    {
        let topic_part = concat!(env!("CARGO_PKG_NAME"), "/repository");
        p(client, topic_part, env!("CARGO_PKG_REPOSITORY")).await?;

        let topic_part = concat!(env!("CARGO_PKG_NAME"), "/version");
        p(client, topic_part, env!("CARGO_PKG_VERSION")).await?;
    }

    let boot_time = System::boot_time()
        .try_into()
        .ok()
        .and_then(|secs| chrono::Local.timestamp_opt(secs, 0).single());
    if let Some(boot_time) = boot_time {
        p(client, "boot-time", boot_time.to_rfc3339()).await?;
    }

    p(client, "os/distribution", System::distribution_id()).await?;
    po(client, "os/name", System::name()).await?;
    po(client, "os/version", System::long_os_version()).await?;
    po(client, "os/kernel", System::kernel_version()).await?;

    p(client, "cpu/arch", System::cpu_arch()).await?;
    po(client, "cpu/cores", System::physical_core_count()).await?;

    {
        let sys =
            System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()));
        let cpus = sys.cpus();

        p(client, "cpu/threads", cpus.len()).await?;

        let mut brands = cpus
            .iter()
            .map(sysinfo::Cpu::brand)
            .map(str::trim)
            .filter(|brand| !brand.is_empty())
            .collect::<Vec<_>>();
        brands.sort_unstable();
        brands.dedup();
        p(client, "cpu/brand", brands.join("; ")).await?;

        let mut vendors = cpus
            .iter()
            .map(sysinfo::Cpu::vendor_id)
            .map(str::trim)
            .filter(|vendor| !vendor.is_empty())
            .collect::<Vec<_>>();
        vendors.sort_unstable();
        vendors.dedup();
        p(client, "cpu/vendor", vendors.join("; ")).await?;
    }

    if let Some(motherboard) = Motherboard::new() {
        po(client, "motherboard/name", motherboard.name()).await?;
        po(client, "motherboard/vendor", motherboard.vendor_name()).await?;
        po(client, "motherboard/version", motherboard.version()).await?;
    }

    po(client, "product/name", Product::name()).await?;
    po(client, "product/family", Product::family()).await?;
    po(
        client,
        "product/stock_keeping_unit",
        Product::stock_keeping_unit(),
    )
    .await?;
    po(client, "product/vendor", Product::vendor_name()).await?;
    po(client, "product/version", Product::version()).await?;

    Ok(())
}

macro_rules! topic {
    ($topic_part:literal) => {{
        const SUFFIX: &str = concat!("/", $topic_part);
        let mut topic = String::with_capacity(HOSTNAME.len() + SUFFIX.len());
        topic.push_str(&HOSTNAME);
        topic.push_str(SUFFIX);
        topic
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
    p(client, topic!("uptime"), uptime).await?;

    let load = System::load_average();
    p(client, topic!("load/one"), load.one).await?;
    p(client, topic!("load/five"), load.five).await?;
    p(client, topic!("load/fifteen"), load.fifteen).await?;

    for comp in Components::new_with_refreshed_list().list() {
        const TOPIC_PART: &str = "/component-temperature/";

        let Some(temp) = comp.temperature().filter(|temp| temp.is_finite()) else {
            continue;
        };
        let label = comp
            .label()
            .trim()
            .replace(|char: char| !char.is_ascii_alphanumeric(), "-");
        let mut topic = String::with_capacity(HOSTNAME.len() + TOPIC_PART.len() + label.len());
        topic.push_str(&HOSTNAME);
        topic.push_str(TOPIC_PART);
        topic.push_str(&label);
        p(client, topic, temp).await?;
    }

    // Ignore errors and override multiple batteries over the same topics
    // Not sure how to handle multiple batteries better and simply using the first wont solve it as the order might be mixed up.
    // Most devices have no or a single battery so thats fine with them?
    if let Ok(batteries) = starship_battery::Manager::new().and_then(|manager| manager.batteries())
    {
        let batteries = batteries
            .flatten()
            .map(|battery| {
                let charge = battery.state_of_charge().value;
                let cycle_count = battery.cycle_count();
                let health = battery.state_of_health().value;
                let state = battery.state();
                (charge, cycle_count, health, state)
            })
            .collect::<Vec<_>>();
        for (charge, cycle_count, health, state) in batteries {
            p(client, topic!("battery/charge"), charge).await?;
            p(client, topic!("battery/state"), state).await?;
            p(client, topic!("battery/health"), health).await?;
            if let Some(cycle_count) = cycle_count {
                let topic = topic!("battery/cycle_count");
                p(client, topic, cycle_count).await?;
            }
        }
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
