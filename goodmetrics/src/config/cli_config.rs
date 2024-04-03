use clap::Parser;
use lazy_static::lazy_static;

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
            .expect("can make a path")
            .to_string(),
        None => "./config.toml".to_string(),
    }
}

pub fn get_args() -> Options {
    let command_line_args = Options::parse();

    let opts = match std::fs::read_to_string(command_line_args.config_file.clone()) {
        Ok(config_file_toml_str) => {
            log::debug!("Using config file: {:?}", config_file_toml_str);
            Options::try_parse_from(config_file_toml_str.lines()).unwrap_or(command_line_args)
            //Options::from_args_with_toml(&config_file_toml_str).unwrap()
        }
        Err(_) => command_line_args,
    };
    log::debug!("Config: {:?}", opts);
    opts
}
