use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct Config {
    temperature_window: Vec<u8>,
    fan_speed_floor: u8,
    fan_speed_ceiling: u8,
}

impl Config {
    fn new(file_path: &str) -> Result<Config, &'static str> {
        let file = File::open(file_path).map_err(|_| "Failed to open config file")?;
        let reader = BufReader::new(file);

        let mut temperature_window = Vec::new();
        let mut fan_speed_floor = 0;
        let mut fan_speed_ceiling = 0;

        for line in reader.lines() {
            let line = line.map_err(|_| "Failed to read line from config file")?;
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() != 2 {
                return Err("Invalid config line format");
            }

            match parts[0].trim() {
                "temperature_window" => {
                    let values: Vec<u8> = parts[1]
                        .split(',')
                        .map(|s| s.trim().parse().unwrap_or(0))
                        .collect();
                    temperature_window = values;
                }
                "fan_speed_floor" => fan_speed_floor = parts[1].trim().parse().unwrap_or(0),
                "fan_speed_ceiling" => fan_speed_ceiling = parts[1].trim().parse().unwrap_or(0),
                _ => return Err("Invalid config option"),
            }
        }

        Ok(Config {
            temperature_window,
            fan_speed_floor,
            fan_speed_ceiling,
        })
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

fn set_fan_speed(speed: u8) {
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
        .spawn()
        .expect("Failed to execute nvidia-settings");

    child.wait().expect("Failed to wait for nvidia-settings");
}

fn adjust_fan_speed(
    temp: u8,
    fan_speed: &mut u8,
    config: &Config,
    last_temp: u8,
    last_fan_speed_increase: &mut Instant,
    last_fan_speed_decrease: &mut Instant,
) {
    if temp > config.temperature_window[config.temperature_window.len() - 1] {
        if *fan_speed < config.fan_speed_ceiling {
            let now = Instant::now();
            if now.duration_since(*last_fan_speed_increase).as_secs() >= 3 {
                *fan_speed += 5;
                *last_fan_speed_increase = now;
            }
        }
    } else if temp < config.temperature_window[0] {
        if *fan_speed > config.fan_speed_floor {
            let now = Instant::now();
            if now.duration_since(*last_fan_speed_decrease).as_secs() >= 10 {
                *fan_speed -= 1;
                *last_fan_speed_decrease = now;
            }
        }
    } else {
        let mut idx = 0;
        for i in 0..config.temperature_window.len() - 1 {
            if temp >= config.temperature_window[i] && temp < config.temperature_window[i + 1] {
                idx = i;
                break;
            }
        }

        if temp > last_temp {
            if *fan_speed < config.fan_speed_ceiling {
                let now = Instant::now();
                if now.duration_since(*last_fan_speed_increase).as_secs() >= 3 {
                    *fan_speed += 5;
                    *last_fan_speed_increase = now;
                }
            }
        } else {
            if *fan_speed > config.fan_speed_floor {
                let now = Instant::now();
                if now.duration_since(*last_fan_speed_decrease).as_secs() >= 10 {
                    *fan_speed -= 1;
                    *last_fan_speed_decrease = now;
                }
            }
        }

        if *fan_speed > config.temperature_window[idx] + 10 {
            *fan_speed = config.temperature_window[idx] + 10;
        }
    }
}

fn main() {
    let config = Config::new("config.txt").unwrap_or_else(|err| {
        println!("Error: {}", err);
        std::process::exit(1);
    });

    let mut fan_speed = get_fan_speed();
    let mut last_temp = get_gpu_temp();
    let mut last_fan_speed_increase = Instant::now();
    let mut last_fan_speed_decrease = Instant::now();

    loop {
        let temp = get_gpu_temp();

        adjust_fan_speed(
            temp,
            &mut fan_speed,
            &config,
            last_temp,
            &mut last_fan_speed_increase,
            &mut last_fan_speed_decrease,
        );

        set_fan_speed(fan_speed);
        last_temp = temp;

        thread::sleep(Duration::from_secs(5));
    }
}
