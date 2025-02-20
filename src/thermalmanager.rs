use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::commands;
use crate::config::Config;
use chrono::prelude::*;

type ThresholdPair = (u64, u64);
type ThresholdWindow = (ThresholdPair, Option<ThresholdPair>);

pub fn get_cur_time() -> String {
    let dt: DateTime<Local> = Local::now();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub struct ThermalManager {
    pub gpu_id: u8,
    pub samples: VecDeque<u64>,
    pub config: Config,
    pub temp_average: u64,
    pub current_temp: u64,
    pub last_adjustment_time: Option<Instant>,
    pub last_temp_time: Option<Instant>,
    pub current_fan_speed: u64,
    pub target_fan_speed: u64,
    pub smooth_mode: String,
}

impl ThermalManager {
    pub fn new(config: Config) -> Self {
        ThermalManager {
            gpu_id: 0,
            samples: VecDeque::with_capacity(config.sampling_window_size),
            config: config.clone(),
            temp_average: 0,
            current_temp: 0,
            last_adjustment_time: None,
            last_temp_time: None,
            current_fan_speed: 0,
            target_fan_speed: config.fan_speed_floor,
            smooth_mode: if config.smooth_mode {
                "~".to_string()
            } else {
                "".to_string()
            },
        }
    }

    pub fn update_temperature(&mut self) {
        self.current_temp = commands::get_gpu_temp(&self.gpu_id);
        self.last_temp_time = Some(Instant::now());
        self.current_fan_speed = commands::get_fan_speed(&self.gpu_id);
        self.samples.push_back(self.current_temp);
        if self.samples.len() > self.config.sampling_window_size {
            self.samples.pop_front();
        }

        // Calculate EMA
        if self.samples.len() < self.config.sampling_window_size {
            // prefer responsiveness until window is full
            self.temp_average = self.current_temp;
        } else {
            self.temp_average = self.calculate_wma();
        }
    }

    pub fn generate_thresholds_and_speeds(&mut self) -> Vec<(u64, u64)> {
        let _temps = self.config.temp_thresholds.clone();
        let _speeds = self.config.fan_speeds.clone();

        _temps.into_iter().zip(_speeds).collect::<Vec<(u64, u64)>>()
    }

    pub fn calculate_wma(&mut self) -> u64 {
        let mut temp_average: f64 = 0.0;
        let mut weight_sum: f64 = 0.0;

        for (i, temp) in self.samples.iter().enumerate() {
            let weight = (self.config.sampling_window_size - i) as f64;
            temp_average += weight * (*temp as f64);
            weight_sum += weight;
        }

        (temp_average / weight_sum) as u64
    }

    pub fn select_nearest_fan_speed(&mut self, thresholds: Vec<(u64, u64)>) -> u64 {
        let mut nearest_speed = self.config.fan_speed_floor;

        // Iterate in reverse to check higher thresholds first
        for (thresh, speed) in thresholds.into_iter().rev() {
            if self.current_temp >= thresh {
                nearest_speed = speed;
                break;
            }
        }

        nearest_speed.clamp(self.config.fan_speed_floor, self.config.fan_speed_ceiling)
    }

    fn get_dwell_time(&mut self) -> bool {
        let dwell_time = Duration::from_secs(self.config.fan_dwell_time);
        if let Some(last_adjust) = self.last_adjustment_time {
            let from_last_adjust = Instant::now().duration_since(last_adjust);
            if from_last_adjust < dwell_time {
                return true;
            }
        }

        false
    }

    fn get_threshold_window(&self, thresholds: &[(u64, u64)]) -> Option<ThresholdWindow> {
        let current_temp = self.current_temp;
        let mut lower_threshold = None;
        let mut upper_threshold = None;

        for &(thresh, speed) in thresholds {
            if thresh <= current_temp {
                if lower_threshold.map_or(true, |(lt, _)| thresh > lt) {
                    lower_threshold = Some((thresh, speed));
                }
            } else if upper_threshold.map_or(true, |(ut, _)| thresh < ut) {
                upper_threshold = Some((thresh, speed));
            }
        }

        match (lower_threshold, upper_threshold) {
            (Some(lower), Some(upper)) => Some((lower, Some(upper))),
            (Some(lower), None) => Some((lower, None)),
            (None, Some(upper)) => Some((upper, None)),
            (None, None) => None,
        }
    }

    pub fn get_smooth_speed(&mut self, thresholds: &[ThresholdPair]) -> u64 {
        let window = self.get_threshold_window(thresholds);

        let current_speed = self.current_fan_speed as f64;
        let max_step = self.config.smooth_mode_max_fan_step as f64;
        let hysteresis = self.config.hysteresis as f64;
        let floor = self.config.fan_speed_floor as f64;
        let ceiling = self.config.fan_speed_ceiling as f64;

        let compute_new_speed = |target_speed: f64| -> u64 {
            let change = target_speed - current_speed;
            let limited_change = if change.abs() <= hysteresis {
                0.0
            } else if change > 0.0 && max_step > 0.0 {
                change.clamp(0.0, max_step)
            } else {
                change.clamp(-max_step, 0.0)
            };

            (current_speed + limited_change)
                .clamp(floor, ceiling)
                .round() as u64
        };

        match window {
            Some(((lower_thresh, lower_speed), Some((upper_thresh, upper_speed)))) => {
                let temp_range = (upper_thresh - lower_thresh) as f64;
                let speed_range = (upper_speed - lower_speed) as f64;
                let temp_diff = (self.current_temp - lower_thresh) as f64;

                let target_speed = lower_speed as f64 + (temp_diff / temp_range) * speed_range;
                compute_new_speed(target_speed)
            }
            Some(((_, lower_speed), None)) => {
                let target_speed = lower_speed as f64;
                compute_new_speed(target_speed)
            }
            None => self.config.fan_speed_floor,
        }
    }

    pub fn get_target_fan_speed(&mut self) -> u64 {
        let thresholds = self.generate_thresholds_and_speeds();

        if self.config.smooth_mode {
            self.target_fan_speed = self.get_smooth_speed(&thresholds);
        } else {
            self.target_fan_speed = self.select_nearest_fan_speed(thresholds.clone());
        }

        self.target_fan_speed
    }

    pub fn set_target_fan_speed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.get_target_fan_speed();

        if self.get_dwell_time() {
            return Ok(()); // Skip adjustment if within dwell time
        }

        if self.current_fan_speed != self.target_fan_speed {
            println!(
                "[{}] Veridian transitioning state: {} C => {} %A -> {}{} %T",
                get_cur_time(),
                self.temp_average,
                self.current_fan_speed,
                self.smooth_mode,
                self.target_fan_speed
            );
            commands::set_fan_speed(&self.gpu_id, self.target_fan_speed)?;
            self.last_adjustment_time = Some(Instant::now());
        }

        Ok(())
    }
}
