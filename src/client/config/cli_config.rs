use lazy_static::lazy_static;
use structopt::StructOpt;
use structopt_toml::StructOptToml;

use super::options::Options;

lazy_static! {
    static ref DEFAULT_DIR: String = default_dir();
}

pub fn default_dir() -> String {
    let home = dirs::home_dir();
    match home {
        Some(home_dir) => home_dir
            .join(".goodmetrics")
            .join("config.toml")
            .to_str()
            .unwrap()
            .to_string(),
        None => "./config.toml".to_string(),
    }
}

pub(crate) fn get_args() -> Options {
    let command_line_args = Options::from_args();

    let opts = match std::fs::read_to_string(command_line_args.config_file.clone()) {
        Ok(config_file_toml_str) => {
            log::debug!("Using config file: {:?}", config_file_toml_str);
            Options::from_args_with_toml(&config_file_toml_str).unwrap()
        }
        Err(_) => command_line_args,
    };
    log::debug!("Config: {:?}", opts);
    opts
}
