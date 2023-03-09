use std::time::Duration;

use clap::Parser;
use serde_derive::Deserialize;

#[derive(Debug, Deserialize, Parser, Clone)]
#[clap(
    author = "Kenny",
    group(
        clap::ArgGroup::new("remote")
            .required(true)
            .multiple(true)
            .args(["connection_string", "otlp_remote"]),
    )
)]
pub struct Options {
    // #[arg(long, help = "A config file")]
    // pub config: Option<String>,
    #[arg(long, default_value = "0.0.0.0:9573", env = "LISTEN_SOCKET_ADDRESS")]
    pub listen_socket_address: String,

    #[arg(long, default_value = "1", env = "MAX_THREADS")]
    pub max_threads: usize,

    #[arg(long, default_value = "debug", env = "LOG_LEVEL")]
    pub log_level: String,

    #[arg(
        long,
        help = "enable the tokio-console listener? (doesn't work on docker)"
    )]
    pub tokio_console: bool,

    #[arg(
        long,
        default_value = "",
        help = "File path to the private key.",
        env = "PRIVATE_KEY_FILE"
    )]
    pub cert_private_key: String,

    #[arg(
        long,
        default_value = "",
        help = "File path to the private key's public certificate.",
        env = "PUBLIC_KEY_FILE"
    )]
    pub cert: String,

    #[arg(
        long,
        default_value = "localhost",
        help = "hostname to use for a self-signed host",
        env = "SELF_SIGNED_HOSTNAME"
    )]
    pub self_signed_hostname: String,

    #[arg(
        long,
        help = "api keys allowed to access the service. If none are supplied, then no extra authorization happens",
        env = "API_KEYS"
    )]
    pub api_keys: Vec<String>,

    #[arg(
        long,
        help = "Example: 7d",
        default_value = "7d",
        env = "TIMESCALE_DEFAULT_RETENTION",
        value_parser = humantime::parse_duration,
    )]
    pub default_retention: Duration,

    #[arg(
        long,
        help = "Example: host=localhost port=2345 user=metrics password=metrics connect_timeout=10",
        env = "TIMESCALE_CONNECTION_STRING"
    )]
    pub connection_string: Option<String>,

    #[arg(
        long,
        help = "Send dumbed down metrics via otel metrics format. Example: https://my.opentelemetry:4317",
        env = "OTLP_ENDPOINT"
    )]
    pub otlp_remote: Option<String>,

    #[arg(
        long,
        help = "Skip server certificate verification for opentelemetry remote",
        env = "OTLP_INSECURE"
    )]
    pub otlp_insecure: bool,
}

pub fn get_args() -> Options {
    let command_line_args = Options::parse();
    log::info!("Args: {:?}", command_line_args);

    command_line_args
}
