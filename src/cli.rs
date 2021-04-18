use clap::{App, AppSettings, Arg};

pub fn build() -> App<'static, 'static> {
    App::new("MQTT Hostname Online")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .global_setting(AppSettings::ColoredHelp)
        .arg(
            Arg::with_name("Host")
                .short("h")
                .long("host")
                .value_name("HOST")
                .takes_value(true)
                .help("Host on which the MQTT Broker is running")
                .default_value("localhost"),
        )
        .arg(
            Arg::with_name("Port")
                .short("p")
                .long("port")
                .value_name("INT")
                .takes_value(true)
                .help("Port on which the MQTT Broker is running")
                .default_value("1883"),
        )
}
