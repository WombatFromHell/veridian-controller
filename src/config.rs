use nix::unistd::{getuid, Uid};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub gpu_id: u8,
    pub temp_thresholds: Vec<u64>,
    pub fan_speeds: Vec<u64>,
    pub fan_speed_floor: u64,
    pub fan_speed_ceiling: u64,
    pub hysteresis: u64,
    pub sampling_window_size: usize,
    pub global_delay: u64,
    pub fan_dwell_time: u64,
    pub smooth_mode: bool,
    pub smooth_mode_incr_weight: f64,
    pub smooth_mode_decr_weight: f64,
    pub smooth_mode_max_fan_step: u64,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    MissingHomeDir,
    MissingConfigFile,
    InvalidDirectory,
    InvalidArrayFormat,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            gpu_id: 0,
            temp_thresholds: vec![48, 58, 68, 78, 86],
            fan_speeds: vec![46, 55, 62, 80, 100],
            fan_speed_floor: 46,
            fan_speed_ceiling: 100,
            sampling_window_size: 10,
            hysteresis: 3,
            global_delay: 2,
            fan_dwell_time: 10,
            smooth_mode: true,
            smooth_mode_incr_weight: 1.0,
            smooth_mode_decr_weight: 2.0,
            smooth_mode_max_fan_step: 10,
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
            ConfigError::InvalidDirectory => write!(f, "Invalid directory"),
            ConfigError::InvalidArrayFormat => write!(
                f,
                "Temperature and Fan Speed arrays must be the same length"
            ),
        }
    }
}
impl std::error::Error for ConfigError {}

pub fn expand_tilde(path: &str) -> Result<PathBuf, ConfigError> {
    if path.starts_with("~/") {
        let home_dir = env::var("HOME").map_err(|_| ConfigError::MissingHomeDir)?;
        let stripped_path = path
            .strip_prefix("~/")
            .ok_or(ConfigError::InvalidDirectory)?;
        Ok(PathBuf::from(home_dir).join(stripped_path))
    } else {
        Ok(PathBuf::from(path))
    }
}

pub fn resolve_path(path: &str) -> Result<PathBuf, ConfigError> {
    let expanded_path = expand_tilde(path)?;
    let expanded_path = Path::new(&expanded_path);

    let resolved_path = if expanded_path.is_absolute() {
        expanded_path.to_path_buf()
    } else {
        let current_dir = env::current_dir().map_err(|_| ConfigError::InvalidDirectory)?;
        current_dir.join(expanded_path)
    };

    if resolved_path.exists() {
        resolved_path.canonicalize().map_err(|e| {
            eprintln!(
                "Failed to canonicalize path '{}': {}",
                resolved_path.display(),
                e
            );
            ConfigError::Io(e)
        })
    } else {
        Ok(resolved_path)
    }
}

pub fn get_config_path(custom_path: Option<String>) -> Result<PathBuf, ConfigError> {
    let path_str = if Uid::is_root(getuid()) {
        custom_path.unwrap_or_else(|| "/etc/veridian-controller.toml".to_string())
    } else {
        let home_dir = env::var("HOME").map_err(|_| ConfigError::MissingHomeDir)?;
        custom_path.unwrap_or_else(|| format!("{}/.config/veridian-controller.toml", home_dir))
    };

    resolve_path(&path_str)
}

impl Config {
    pub fn new(custom_path: Option<String>) -> Result<Config, ConfigError> {
        let file_path = get_config_path(custom_path)?;

        let mut file = match File::open(&file_path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Err(ConfigError::MissingConfigFile)
            }
            Err(e) => return Err(ConfigError::Io(e)),
        };

        println!("Using config file: {}", file_path.display());

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(ConfigError::Io)?;

        let config: Self = toml::from_str(&contents).map_err(ConfigError::Toml)?;

        if config.fan_speeds.len() != config.temp_thresholds.len() {
            return Err(ConfigError::InvalidArrayFormat);
        }

        Ok(config)
    }

    pub fn write_to_file(&self, custom_path: Option<String>) -> Result<(), ConfigError> {
        let file_path = get_config_path(custom_path)?;

        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    eprintln!("Failed to create directory '{}': {}", parent.display(), e);
                    ConfigError::Io(e)
                })?;
            }
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .map_err(ConfigError::Io)?;

        let config_string = toml::to_string(self).unwrap();
        file.write_all(config_string.as_bytes())
            .map_err(ConfigError::Io)?;

        Ok(())
    }
}

pub fn load_config_from_env(custom_path: Option<String>) -> Result<Config, ConfigError> {
    let config_path = get_config_path(custom_path.clone());
    let config = match config_path {
        Ok(_) => Config::new(custom_path.clone()),
        Err(e) => Err(e),
    };

    match config {
        Ok(c) => Ok(c),
        Err(ConfigError::MissingConfigFile) => {
            let default_config = Config::default();
            default_config.write_to_file(custom_path.clone())?;
            let resolved_path = get_config_path(custom_path.clone())?;
            println!(
                "No configuration file found!\nCreated a new config at: {}...",
                resolved_path.display()
            );
            Ok(default_config)
        }
        Err(ConfigError::Toml(_)) => {
            let default_config = Config::default();
            default_config.write_to_file(custom_path.clone())?;
            let resolved_path = get_config_path(custom_path.clone())?;
            println!(
                "Invalid configuration file detected!\nRecreating a default config at: {}...",
                resolved_path.display()
            );
            Ok(default_config)
        }
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    }
}
