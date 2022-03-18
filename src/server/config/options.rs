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

    // File path to the private key.
    #[clap(long, default_value = "")]
    pub cert_private_key: String,
    // File path to the private key's public certificate.
    #[clap(long, default_value = "")]
    pub cert: String,
    // hostname to use for a self-signed host
    #[clap(long, default_value = "localhost")]
    pub self_signed_hostname: String,

    #[clap(
        long,
        help = "Example: host=localhost port=2345 user=metrics password=metrics connect_timeout=10"
    )]
    pub connection_string: Option<String>,

    #[clap(long, help = "Example: https://my.opentelemetry:4317")]
    pub otlp_remote: Option<String>,
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
