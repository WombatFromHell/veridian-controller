use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use signal_hook::{iterator::Signals, consts::SIGINT};
use std::error::Error;

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    temperatures: Vec<u8>,
    fan_speeds: Vec<u8>,
    fan_speed_floor: u8,
    fan_speed_ceiling: u8,
    fan_hysteresis: u8,
    window_size: usize,
    global_timer: u64,
    smooth_mode: bool,
}

#[derive(Debug)]
enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    MissingHomeDir,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            temperatures: vec![32, 48, 58, 68, 78, 88],
            fan_speeds: vec![0, 30, 55, 65, 80, 100],
            fan_speed_floor: 30,
            fan_speed_ceiling: 100,
            fan_hysteresis: 3,
            smooth_mode: true,
            window_size: 5,
            global_timer: 2,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "IO error: {}", err),
            ConfigError::Toml(err) => write!(f, "TOML parse error: {}", err),
            ConfigError::MissingHomeDir => write!(f, "Missing HOME directory"),
        }
    }
}
impl std::error::Error for ConfigError {}

impl Config {
    fn new() -> Result<Self, ConfigError> {
        let home_dir = env::var("HOME").ok().ok_or(ConfigError::MissingHomeDir)?;
        let cwd_config_path = "veridian-controller.toml";
        let xdg_config_path = format!("{}/.config/veridian-controller.toml", home_dir);

        let mut file = if let Ok(file) = File::open(cwd_config_path) {
            println!("Loaded config from: {}", &cwd_config_path);
            file
        } else {
            match File::open(&xdg_config_path) {
                Ok(file) => {
                    println!("Loaded config from: {}", &xdg_config_path);
                    file
                }
                Err(_) => {
                    println!("Failed to load config, using defaults!");
                    return Ok(Self::default());
                }
            }
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(ConfigError::Io)?;

        let config: Self = toml::from_str(&contents).map_err(ConfigError::Toml)?;

        Ok(config)
    }
}

fn get_gpu_temp() -> u8 {
    let output = Command::new("nvidia-smi")
        .args(&["--query-gpu=temperature.gpu", "--format=csv,noheader"])
        .output()
        .expect("Failed to execute nvidia-smi");

    let temp_str = String::from_utf8_lossy(&output.stdout);
    temp_str.trim().parse().unwrap_or(0)
}

fn get_fan_speed() -> u8 {
    let output = Command::new("nvidia-smi")
        .args(&["--query-gpu=fan.speed", "--format=csv,noheader"])
        .output()
        .expect("Failed to execute nvidia-smi");

    let speed_str = String::from_utf8_lossy(&output.stdout);
    let speed_str = speed_str.trim().replace(" %", "");
    speed_str.parse().unwrap_or(0)
}

fn set_fan_control(mode: u8) {
    let mut child = Command::new("sudo")
        .args(&["nvidia-settings", "-a", format!("*:1[gpu:0]/GPUFanControlState={}", mode).as_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute nvidia-settings");

    child.wait().expect("Failed to wait for nvidia-settings");
}

fn set_fan_speed(speed: u8) {
    println!("Setting fan speed: {} %", speed);

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

fn adjust_fan_speed(temp: u8, fan_speed: &mut u8, config: &Config, samples: &mut VecDeque<u32>) {
    *fan_speed = (*fan_speed).clamp(0, config.fan_speed_ceiling);

    samples.push_back(temp as u32);
    if samples.len() > config.window_size {
        samples.reserve(1);
        samples.pop_front();
    }
    let smooth_temp: u32 = (samples.iter().sum::<u32>() as f32 / samples.len() as f32).round() as u32;

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
        if config.smooth_mode {
            println!(
                "GPU Temp [Avg]: {} C / Fan Speed: {} %",
                smooth_temp, fan_speed
            );
        } else {
            println!("GPU Temp: {} C / Fan Speed: {} %", temp, fan_speed);
        }
        // Adjust fan speed based on the closest temperature in the temperature table
        let target_fan_speed = config.fan_speeds[index];
        *fan_speed = target_fan_speed.clamp(0, config.fan_speed_ceiling);
        set_fan_speed(*fan_speed);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut signals = Signals::new(&[SIGINT])?;
    thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGINT => {
                    println!("Caught SIGINT, cleaning up...");
                    set_fan_control(0 as u8);
                    std::process::exit(0);
                }
                _ => (),
            }
            println!("Caught signal: {:?}", signal);
            std::process::exit(0);
        }
    });

    let config = Config::new().unwrap_or_else(|err| {
        println!("Error: {}", err);
        std::process::exit(1);
    });
    if config.fan_speeds.len() != config.temperatures.len() {
        println!("Error: fan_speeds and temperatures arrays must be the same length!");
        std::process::exit(1);
    }

    let mut fan_speed = get_fan_speed();
    let mut samples: VecDeque<u32> = VecDeque::with_capacity(config.window_size);

    loop {
        let temp = get_gpu_temp();

        set_fan_control(1 as u8);
        adjust_fan_speed(temp, &mut fan_speed, &config, &mut samples);

        thread::sleep(Duration::from_secs(config.global_timer));
    }
}
