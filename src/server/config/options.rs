use clap::Parser;
use serde_derive::Deserialize;

#[derive(Debug, Deserialize, Parser)]
#[clap(author = "Kenny")]
pub struct Options {
    #[clap(long, help = "A config file")]
    pub config: Option<String>,
    #[clap(long, default_value = "0.0.0.0:9573")]
    pub listen_socket_address: String,
    #[clap(long, default_value = "1")]
    pub max_threads: usize,
    #[clap(long, default_value = "debug")]
    pub log_level: String,
    #[clap(
        long,
        help = "enable the tokio-console listener? (doesn't work on docker)"
    )]
    pub tokio_console: bool,

    #[clap(long, default_value = "", help = "File path to the private key.")]
    pub cert_private_key: String,
    #[clap(
        long,
        default_value = "",
        help = "File path to the private key's public certificate."
    )]
    pub cert: String,
    #[clap(
        long,
        default_value = "localhost",
        help = "hostname to use for a self-signed host"
    )]
    pub self_signed_hostname: String,
    #[clap(
        long,
        help = "api keys allowed to access the service. If none are supplied, then no extra authorization happens"
    )]
    pub api_keys: Vec<String>,

    #[clap(
        long,
        help = "Example: host=localhost port=2345 user=metrics password=metrics connect_timeout=10"
    )]
    pub connection_string: Option<String>,

    #[clap(
        long,
        help = "Send dumbed down metrics via otel metrics format. Example: https://my.opentelemetry:4317"
    )]
    pub otlp_remote: Option<String>,
    #[clap(
        long,
        help = "Skip server certificate verification for opentelemetry remote"
    )]
    pub otlp_insecure: bool,
}

pub fn get_args() -> Options {
    let command_line_args = Options::parse();
    log::info!("Args: {:?}", command_line_args);

    // I don't know how to do this
    // match &command_line_args.config {
    //     Some(config_file_path) => {
    //         let toml_str = std::fs::read_to_string(config_file_path).unwrap();
    //         log::info!("reading config from {}", toml_str);
    //        Options::try_parse_from(toml_str.split_whitespace().filter(|l| !l.is_empty()) ).unwrap()
    //        Options::from_args_with_toml(&toml_str).unwrap()
    //     }
    //     None => command_line_args,
    // }
    command_line_args
}
