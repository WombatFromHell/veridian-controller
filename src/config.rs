use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::io::{ErrorKind, Read};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub temp_thresholds: Vec<u32>,
    pub fan_speeds: Vec<u32>,
    pub fan_speed_floor: u32,
    pub fan_speed_ceiling: u32,
    pub incr_weight: f32,
    pub decr_weight: f32,
    pub hysteresis: u32,
    pub sampling_window_size: usize,
    pub global_delay: u64,
    pub post_adjust_delay: u64,
    pub smooth_mode: bool,
    pub smooth_mode_fan_step: u32,
    pub debug_mode: bool,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    MissingHomeDir,
    MissingConfigFile,
    InvalidArrayFormat,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            temp_thresholds: vec![53, 63, 73, 86],
            fan_speeds: vec![46, 62, 80, 100],
            fan_speed_floor: 30,
            fan_speed_ceiling: 100,
            incr_weight: 0.5,
            decr_weight: 0.2,
            hysteresis: 3,
            sampling_window_size: 5,
            global_delay: 2,
            post_adjust_delay: 4,
            smooth_mode: false,
            smooth_mode_fan_step: 5,
            debug_mode: false,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "IO error: {}", err),
            ConfigError::Toml(err) => write!(f, "TOML parse error: {}", err),
            ConfigError::MissingHomeDir => write!(f, "Missing HOME directory"),
            ConfigError::MissingConfigFile => write!(f, "Missing configuration file"),
            ConfigError::InvalidArrayFormat => write!(
                f,
                "Temperature and Fan Speed arrays must be the same length"
            ),
        }
    }
}
impl std::error::Error for ConfigError {}

impl Config {
    pub fn new(custom_path: Option<String>) -> Result<Self, ConfigError> {
        let home_dir = env::var("HOME").map_err(|_| ConfigError::MissingHomeDir)?;
        let xdg_config_path = format!("{}/.config/veridian-controller.toml", home_dir);

        let file_path = custom_path.or_else(|| Some(xdg_config_path));

        let file_path = file_path.ok_or(ConfigError::MissingConfigFile)?;

        let mut file = File::open(&file_path).map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                ConfigError::MissingConfigFile
            } else {
                ConfigError::Io(e)
            }
        })?;
        println!("Using config file: {}", file_path);

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(ConfigError::Io)?;

        let config: Self = toml::from_str(&contents).map_err(ConfigError::Toml)?;

        if config.fan_speeds.len() != config.temp_thresholds.len() {
            return Err(ConfigError::InvalidArrayFormat);
        }

        Ok(config)
    }

    pub fn write_to_file(&self) -> Result<(), ConfigError> {
        let home_dir = env::var("HOME").map_err(|_| ConfigError::MissingHomeDir)?;
        let xdg_config_path = format!("{}/.config/veridian-controller.toml", home_dir);

        let mut file = File::create(&xdg_config_path).map_err(|e| {
            if e.kind() == ErrorKind::PermissionDenied {
                ConfigError::Io(e)
            } else {
                ConfigError::Io(e)
            }
        })?;

        let config_string = toml::to_string(self).unwrap();

        file.write_all(config_string.as_bytes())
            .map_err(ConfigError::Io)?;

        Ok(())
    }
}

pub fn load_config_from_env(custom_path: Option<String>) -> Result<Config, ConfigError> {
    let config = Config::new(custom_path).unwrap_or_else(|err| {
        match err {
            ConfigError::MissingConfigFile => {
                println!("Error: No configuration file found!");
                println!(
                    "Writing a default configuration file to ~/.config/veridian-controller.toml..."
                );
                println!("Please rerun the program to use the new configuration file.");
                Config::default().write_to_file().unwrap();
            }
            ConfigError::Io(err) => {
                println!("Error: {}", err);
            }
            ConfigError::Toml(err) => {
                println!("Error: {}", err);
            }
            ConfigError::MissingHomeDir => {
                println!("Error: HOME environment variable not set!");
            }
            ConfigError::InvalidArrayFormat => {
                println!("Error: fan_speeds and temp_thresholds arrays must be the same length!");
            }
        }
        std::process::exit(1);
    });

    Ok(config)
}

