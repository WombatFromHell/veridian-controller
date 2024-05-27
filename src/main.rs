use chrono::prelude::*;
use clap::Parser;
use serde::{Deserialize, Serialize};
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the config file to load
    #[arg(short, long, value_name = "PATH")]
    file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    temperatures: Vec<u8>,
    fan_speeds: Vec<u32>,
    fan_speed_floor: u32,
    fan_speed_ceiling: u32,
    fan_hysteresis: u32,
    window_size: usize,
    global_timer: u64,
    disable_lo_temp_fan_control: bool,
    smooth_mode: bool,
}

#[derive(Debug)]
enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    MissingHomeDir,
    MissingConfigFile,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            temperatures: vec![48, 58, 68, 78, 88],
            fan_speeds: vec![30, 55, 65, 80, 100],
            fan_speed_floor: 30,
            fan_speed_ceiling: 100,
            fan_hysteresis: 3,
            window_size: 5,
            global_timer: 2,
            smooth_mode: true,
            disable_lo_temp_fan_control: true,
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
        }
    }
}
impl std::error::Error for ConfigError {}

impl Config {
    fn new(custom_path: Option<String>) -> Result<Self, ConfigError> {
        let home_dir = env::var("HOME").map_err(|_| ConfigError::MissingHomeDir)?;
        let cwd_config_path = "veridian-controller.toml";
        let xdg_config_path = format!("{}/.config/veridian-controller.toml", home_dir);

        let file_path = custom_path.or_else(|| {
            if Path::new(cwd_config_path).is_file() {
                Some(cwd_config_path.to_owned())
            } else {
                Some(xdg_config_path)
            }
        });

        let file_path = file_path.ok_or(ConfigError::MissingConfigFile)?;

        let mut file = File::open(&file_path).map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                ConfigError::MissingConfigFile
            } else {
                ConfigError::Io(e)
            }
        })?;

        println!("Loaded config from: {}", file_path);

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(ConfigError::Io)?;

        let config: Self = toml::from_str(&contents).map_err(ConfigError::Toml)?;

        Ok(config)
    }
}

fn get_gpu_temp() -> u32 {
    let output = Command::new("nvidia-smi")
        .args(&["--query-gpu=temperature.gpu", "--format=csv,noheader"])
        .output()
        .expect("Failed to execute nvidia-smi");

    let temp_str = String::from_utf8_lossy(&output.stdout);

    temp_str
        .trim()
        .parse::<u32>()
        .unwrap_or(0 as u32)
        .clamp(0, 200)
}

fn get_fan_speed() -> u32 {
    let output = Command::new("nvidia-smi")
        .args(&["--query-gpu=fan.speed", "--format=csv,noheader"])
        .output()
        .expect("Failed to execute nvidia-smi");

    let _speed_str = String::from_utf8_lossy(&output.stdout);
    let speed_str = _speed_str.trim().replace(" %", "");

    speed_str.parse::<u32>().unwrap_or(0 as u32).clamp(0, 100)
}

fn set_fan_control(mode: u8) {
    let mut child = Command::new("sudo")
        .args(&[
            "nvidia-settings",
            "-a",
            format!("*:1[gpu:0]/GPUFanControlState={}", mode).as_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute nvidia-settings");

    child.wait().expect("Failed to wait for nvidia-settings");
}

fn set_fan_speed(speed: u32) {
    let mut child = Command::new("sudo")
        .args(&[
            "nvidia-settings",
            "-a",
            "*:1[gpu:0]/GPUFanControlState=1",
            "-a",
            &format!("*:1[fan-0]/GPUTargetFanSpeed={}", speed),
            "-a",
            &format!("*:1[fan-1]/GPUTargetFanSpeed={}", speed),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("Failed to execute nvidia-settings");

    child.wait().expect("Failed to wait for nvidia-settings");
}

fn get_cur_time() -> String {
    let dt: DateTime<Local> = Local::now();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn adjust_fan_speed(temp: u32, fan_speed: u32, config: &Config, samples: &mut VecDeque<u32>) {
    // fan speed floor cannot go below 30 on modern GPUs
    let _fan_speed_floor = config.fan_speed_floor.clamp(30, 100);
    let _fan_speed_ceiling = config.fan_speed_ceiling.clamp(30, 100);
    let _fan_speed = fan_speed.to_owned().clamp(0, 100);

    samples.push_back(temp as u32);
    if samples.len() > config.window_size {
        samples.reserve(1);
        samples.pop_front();
    }
    let smooth_temp: u32 =
        (samples.iter().sum::<u32>() as f32 / samples.len() as f32).round() as u32;

    // Find the closest temperature match for adjusting fan speed
    let mut closest_temp_index = None;
    let mut closest_diff = u32::MAX;

    for (i, &t) in config.temperatures.iter().enumerate() {
        let mut diff: u32 = (temp as i32 - t as i32).abs() as u32;
        // use average temperature in smooth_mode
        if config.smooth_mode {
            diff = (smooth_temp as i32 - t as i32).abs() as u32;
        }

        if diff < closest_diff {
            closest_diff = diff;
            closest_temp_index = Some(i);
        }
    }

    if let Some(index) = closest_temp_index {
        // Adjust fan speed based on the closest temperature in the temperature table
        let target_fan_speed = config.fan_speeds[index].clamp(30, 100);
        let fan_hysteresis_hi = (_fan_speed + config.fan_hysteresis).clamp(30, 100);
        let fan_hysteresis_lo = _fan_speed.abs_diff(config.fan_hysteresis).clamp(30, 100);
        let cur_time = get_cur_time();

        if target_fan_speed > fan_hysteresis_hi || target_fan_speed < fan_hysteresis_lo {
            if config.smooth_mode {
                println!(
                    "[{}] GPU Temp [Avg]: {} C / Fan Speed: {} % -> {} %",
                    cur_time, smooth_temp, _fan_speed, target_fan_speed
                );
            } else {
                println!(
                    "[{}] GPU Temp: {} C / Fan Speed: {} % -> {} %",
                    cur_time, temp, _fan_speed, target_fan_speed
                );
            }

            set_fan_speed(target_fan_speed);
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGINT | SIGTERM => {
                    println!("Caught graceful shutdown, cleaning up...");
                    set_fan_control(0);
                    std::process::exit(1);
                }
                _ => {
                    println!("Caught signal: {:?}", signal);
                    std::process::exit(0)
                }
            }
        }
    });

    let config = Config::new(args.file).unwrap_or_else(|err| {
        println!("Error: {}", err);
        std::process::exit(1);
    });
    if config.fan_speeds.len() != config.temperatures.len() {
        println!("Error: fan_speeds and temperatures arrays must be the same length!");
        std::process::exit(1);
    }

    let mut samples: VecDeque<u32> = VecDeque::with_capacity(config.window_size);
    set_fan_control(1);

    loop {
        let fan_speed = get_fan_speed();
        let temp = get_gpu_temp();

        adjust_fan_speed(temp, fan_speed, &config, &mut samples);

        thread::sleep(Duration::from_secs(config.global_timer));
    }
}
