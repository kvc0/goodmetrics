use serde_derive::Deserialize;
use structopt::StructOpt;
use structopt_toml::StructOptToml;

#[derive(Debug, Deserialize, StructOpt, StructOptToml)]
#[serde(default)]
pub(crate) struct Options {
    #[structopt(long)] pub config: Option<String>,
    #[structopt(long, default_value = "0.0.0.0:9573")] pub listen_socket_address: String,
    #[structopt(long, default_value = "1")] pub max_threads: usize,
    #[structopt(long, default_value = "debug")] pub log_level: String,

    // File path to the private key.
    #[structopt(long, default_value = "")] pub cert_private_key: String,
    // File path to the private key's public certificate.
    #[structopt(long, default_value = "")] pub cert: String,
    // hostname to use for a self-signed host
    #[structopt(long, default_value = "localhost")] pub self_signed_hostname: String,

    #[structopt(long, default_value = "host=localhost port=2345 user=metrics password=metrics connect_timeout=10")] pub connection_string: String,
}

pub(crate) fn get_args() -> Options {
    let command_line_args = Options::from_args();

    match command_line_args.config {
        Some(config_file_path) => {
            let toml_str = std::fs::read_to_string(config_file_path).unwrap();
            Options::from_args_with_toml(&toml_str).unwrap()
        },
        None => {
            command_line_args
        },
    }
}
