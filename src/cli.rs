use clap::{command, value_parser, Arg, Command, ValueHint};

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn build() -> Command<'static> {
    command!()
        .arg(
            Arg::new("broker")
                .short('b')
                .long("broker")
                .env("MQTT_BROKER")
                .value_hint(ValueHint::Hostname)
                .value_name("HOST")
                .takes_value(true)
                .help("Host on which the MQTT Broker is running"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .env("MQTT_PORT")
                .value_hint(ValueHint::Other)
                .value_name("INT")
                .takes_value(true)
                .value_parser(value_parser!(u16))
                .default_value("1883")
                .help("Port on which the MQTT Broker is running"),
        )
        .arg(
            Arg::new("username")
                .short('u')
                .long("username")
                .env("MQTT_USERNAME")
                .value_hint(ValueHint::Username)
                .value_name("STRING")
                .takes_value(true)
                .requires("password")
                .help("Username to access the MQTT broker")
                .long_help(
                    "Username to access the MQTT broker. Anonymous access when not supplied.",
                ),
        )
        .arg(
            Arg::new("password")
                .long("password")
                .env("MQTT_PASSWORD")
                .value_hint(ValueHint::Other)
                .value_name("STRING")
                .hide_env_values(true)
                .takes_value(true)
                .requires("username")
                .help("Password to access the MQTT broker")
                .long_help(
                    "Password to access the MQTT broker. Passing the password via command line is insecure as the password can be read from the history!",
                ),
        )
}

#[test]
fn verify() {
    build().debug_assert();
}
