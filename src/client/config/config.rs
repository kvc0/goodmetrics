use serde_derive::Deserialize;
use structopt::{StructOpt};
use structopt_toml::StructOptToml;
use lazy_static::lazy_static;

use crate::metrics::Datum;

lazy_static! {
    static ref DEFAULT_DIR: String = default_dir();
}

fn default_dir() -> String {
    let home = dirs::home_dir();
    match home {
        Some(home_dir) => {
            home_dir.join(".goodmetrics").join("config.toml").to_str().unwrap().to_string()
        },
        None => {
            "./config.toml".to_string()
        },
    }
}

#[derive(Debug, Deserialize, StructOpt, StructOptToml)]
#[serde(default)]
#[structopt(about = "Good metrics CLI client")]
pub(crate) struct Options {
    #[structopt(long, default_value = &DEFAULT_DIR)] pub config_file: String,
    #[structopt(long, default_value = "http://127.0.0.1:9573")] pub goodmetrics_server: String,
    #[structopt(long, default_value = "debug")] pub log_level: String,

    #[structopt(subcommand)]
    pub command: Subcommand,
}

#[derive(Debug, Deserialize, StructOpt)]
pub(crate) enum Subcommand {
    #[structopt(about = "Send measurements")]
    Send {
        #[structopt(parse(try_from_str = serde_json::from_str))]
        metrics: Vec<Datum>,
    },
}

pub(crate) fn get_args() -> Options {
    let command_line_args = Options::from_args();

    let opts = match std::fs::read_to_string(command_line_args.config_file.clone()) {
        Ok(config_file_toml_str) => {
            log::debug!("Using config file: {:?}", config_file_toml_str);
            Options::from_args_with_toml(&config_file_toml_str).unwrap()
        },
        Err(_) => {
            command_line_args
        },
    };
    log::debug!("Config: {:?}", opts);
    opts
}
