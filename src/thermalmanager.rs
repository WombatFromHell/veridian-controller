use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::commands;
use crate::config::Config;
use crate::helpers;

pub struct ThermalManager<'a> {
    samples: VecDeque<u32>,
    temp_thresholds: Vec<u32>,
    fan_speeds: Vec<u32>,
    config: &'a Config,
    last_adjustment_time: Option<Instant>,
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
            target_fan_speed: config.fan_speed_floor,
        }
    }

    pub fn update_temperature(&mut self) {
        let current_temp = commands::get_gpu_temp();
        self.samples.push_back(current_temp);
        if self.samples.len() > self.config.sampling_window_size {
            self.samples.pop_front();
        }
    }

    pub fn calculate_target_fan_speed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let current_fan_speed = commands::get_fan_speed();
        let temp_average = self.samples.iter().sum::<u32>() / self.samples.len() as u32;
        let now = std::time::Instant::now();

        // Sort the temperature thresholds and fan speeds in ascending order
        let mut sorted_temp_thresholds = self.temp_thresholds.clone();
        let mut sorted_fan_speeds = self.fan_speeds.clone();
        sorted_temp_thresholds.sort_unstable();
        sorted_fan_speeds.sort_unstable();

        let target_speed = sorted_temp_thresholds
            .iter()
            .zip(sorted_fan_speeds.iter())
            .filter(|(temp, _)| **temp <= temp_average)
            .max_by_key(|(temp, _)| *temp)
            .map_or(self.config.fan_speed_floor, |(_, speed)| *speed);

        if self.config.smooth_mode {
            self.target_fan_speed = self.get_smooth_speed(current_fan_speed, target_speed);
        } else {
            self.target_fan_speed = target_speed;
        }

        if self.config.debug_mode && current_fan_speed != self.target_fan_speed {
            println!(
                "[{}] DEBUG: ThermalManager got {} C, transitioning fan speed: {} % -> {} %",
                helpers::get_cur_time(),
                temp_average,
                current_fan_speed,
                self.target_fan_speed
            );
        }
        self.adjust_target_fan_speed(current_fan_speed, self.target_fan_speed, now)?;

        Ok(())
    }

    fn adjust_target_fan_speed(
        &mut self,
        current_speed: u32,
        target_speed: u32,
        now: Instant,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If the new target speed is the same as the current fan speed, return early
        if target_speed == current_speed {
            return Ok(()); // wait fan speed needs to be set
        }

        let cooldown_elapsed = self.last_adjustment_time.map_or(true, |last_time| {
            now.duration_since(last_time) >= Duration::from_secs(self.config.post_adjust_delay)
        });
        if cooldown_elapsed {
            if self.config.debug_mode {
                println!("Setting target fan speed: {} %", self.target_fan_speed);
            }
            commands::set_fan_speed(self.target_fan_speed)?;
        }

        Ok(())
    }

    fn get_smooth_speed(&self, current_speed: u32, target_speed: u32) -> u32 {
        let step_size = self.config.smooth_mode_fan_step;
        let smooth_speed = current_speed;
        let speed_diff = target_speed.wrapping_sub(current_speed);
        let abs_diff = speed_diff.max(target_speed.wrapping_sub(current_speed));
        let steps_needed = (abs_diff / step_size) + (abs_diff % 2 != 0) as u32;
        let direction = if target_speed > current_speed { 1 } else { -1 };
        let adjusted_smooth_speed = smooth_speed as i32 + (steps_needed as i32) * direction;
        adjusted_smooth_speed.clamp(
            self.config.fan_speed_floor as i32,
            self.config.fan_speed_ceiling as i32,
        ) as u32
    }
}
