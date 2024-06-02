use std::cmp::{max, min};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::commands;
use crate::config::Config;
use crate::helpers;

const MIN_FAN_SPEED: u32 = 30;
const MAX_FAN_SPEED: u32 = 30;

pub struct ThermalManager<'a> {
    samples: VecDeque<u32>,
    temp_thresholds: Vec<u32>,
    fan_speeds: Vec<u32>,
    config: &'a Config,
    last_adjustment_time: Option<Instant>,
    last_smooth_adjust: Option<Instant>,
    current_temp: u32,
    current_fan_speed: u32,
    target_fan_speed: u32,
}

impl<'a> ThermalManager<'a> {
    pub fn new(config: &'a Config) -> Self {
        ThermalManager {
            samples: VecDeque::with_capacity(config.sampling_window_size),
            temp_thresholds: config.temp_thresholds.to_owned(),
            fan_speeds: config.fan_speeds.to_owned(),
            config,
            last_adjustment_time: None,
            last_smooth_adjust: None,
            current_temp: 0,
            current_fan_speed: 0,
            target_fan_speed: config.fan_speed_floor,
        }
    }

    pub fn update_temperature(&mut self) {
        self.current_temp = commands::get_gpu_temp();
        self.current_fan_speed = commands::get_fan_speed();
        self.samples.push_back(self.current_temp);
        if self.samples.len() > self.config.sampling_window_size {
            self.samples.pop_front();
        }
    }

    pub fn calculate_target_fan_speed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let temp_average = self.samples.iter().sum::<u32>() / self.samples.len() as u32;
        let now = std::time::Instant::now();
        let cooldown_elapsed = self.last_adjustment_time.map_or(true, |last_time| {
            now.duration_since(last_time) >= Duration::from_secs(self.config.post_adjust_delay)
        });

        // Sort the temperature thresholds and fan speeds in ascending order
        let mut sorted_temp_thresholds = self.temp_thresholds.clone();
        let mut sorted_fan_speeds = self.fan_speeds.clone();
        sorted_temp_thresholds.sort_unstable();
        sorted_fan_speeds.sort_unstable();

        let target_speed = {
            let mut target_speed_idx = 0;
            for (i, threshold) in sorted_temp_thresholds.iter().enumerate() {
                if *threshold >= temp_average {
                    target_speed_idx = i;
                    break;
                }
            }
            sorted_fan_speeds[target_speed_idx]
        };

        let mut smooth_mode = "";
        let cur_speed_hyst_hi = self.current_fan_speed + self.config.hysteresis;
        let cur_speed_hyst_lo = self.current_fan_speed + self.config.hysteresis;
        if self.config.smooth_mode {
            self.target_fan_speed =
                self.get_smooth_speed(self.current_fan_speed, target_speed, now);
            smooth_mode = "~";
        } else if target_speed > cur_speed_hyst_hi || target_speed < cur_speed_hyst_lo {
            // generally post-Pascal GPUs cannot go below 30% fan speed
            // ... or above 80% / 100% depending on the generation and maker
            self.target_fan_speed = target_speed.clamp(MIN_FAN_SPEED, MAX_FAN_SPEED);
        }

        if self.current_fan_speed != self.target_fan_speed && cooldown_elapsed {
            println!(
                "[{}] Veridian transitioning state: {} C => {} %A -> {}{} %T",
                helpers::get_cur_time(),
                temp_average,
                self.current_fan_speed,
                smooth_mode,
                self.target_fan_speed
            );
            commands::set_fan_speed(self.target_fan_speed)?;
            self.last_adjustment_time = Some(now);
        }

        Ok(())
    }

    fn get_smooth_speed(&mut self, current_speed: u32, target_speed: u32, now: Instant) -> u32 {
        let smooth_dwell_time = Duration::from_secs(self.config.smooth_mode_dwell_time);
        if let Some(last_adjust) = self.last_smooth_adjust {
            if now.duration_since(last_adjust) < smooth_dwell_time {
                return current_speed; // Skip adjustment if within dwell time
            }
        }

        let lower_threshold = target_speed - self.config.hysteresis / 2;
        let upper_threshold = target_speed + self.config.hysteresis / 2;

        let mut adjusted_speed = current_speed;
        let step_size = self.config.smooth_mode_fan_step;

        if current_speed < lower_threshold {
            adjusted_speed = min(current_speed + step_size, upper_threshold);
        } else if current_speed > upper_threshold {
            adjusted_speed = max(current_speed - step_size, lower_threshold);
        }

        // don't let speed fall below practical lower boundry
        let min_speed = if self.config.fan_speed_floor <= MIN_FAN_SPEED {
            MIN_FAN_SPEED
        } else {
            self.config.fan_speed_floor
        };

        self.last_smooth_adjust = Some(now);
        adjusted_speed.clamp(min_speed, self.config.fan_speed_ceiling)
    }
}
