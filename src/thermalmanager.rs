use std::cmp::{max, min};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::commands;
use crate::config::Config;
use crate::helpers;

pub struct ThermalManager<'a> {
    samples: VecDeque<u32>,
    config: &'a Config,
    temp_average: u32,
    current_temp: u32,
    last_temp: u32,
    last_adjustment_time: Option<Instant>,
    last_temp_time: Option<Instant>,
    current_fan_speed: u32,
    target_fan_speed: u32,
    smooth_mode: &'a str,
}

impl<'a> ThermalManager<'a> {
    pub fn new(config: &'a Config) -> Self {
        ThermalManager {
            samples: VecDeque::with_capacity(config.sampling_window_size),
            config,
            temp_average: 0,
            current_temp: 0,
            last_temp: 0,
            last_adjustment_time: None,
            last_temp_time: None,
            current_fan_speed: 0,
            target_fan_speed: config.fan_speed_floor,
            smooth_mode: if config.smooth_mode { "~" } else { "" },
        }
    }

    pub fn update_temperature(&mut self) {
        self.current_temp = commands::get_gpu_temp();
        self.last_temp_time = Some(Instant::now());
        self.current_fan_speed = commands::get_fan_speed();
        self.samples.push_back(self.current_temp);
        if self.samples.len() > self.config.sampling_window_size {
            self.samples.pop_front();
        }
        self.temp_average = self.samples.iter().sum::<u32>() / self.samples.len() as u32;
    }

    fn select_nearest_fan_speed(&mut self, temperature: u32) -> u32 {
        // create an array of descending tuples and return matching speed directly
        let mut nearest_speed = self.config.fan_speed_floor;
        let _temps = self.config.temp_thresholds.clone();
        let _speeds = self.config.fan_speeds.clone();
        let _rev_tuples = _temps
            .into_iter()
            .zip(_speeds.into_iter())
            .rev()
            .collect::<Vec<(u32, u32)>>();

        for (thresh, speed) in _rev_tuples.iter() {
            let hyst_hi = thresh.saturating_add(self.config.hysteresis);
            // prefer the higher threshold to reduce overheating
            if temperature >= hyst_hi {
                nearest_speed = *speed;
                break;
            }
        }

        // NOTE: generally post-Pascal GPUs cannot go below 30% fan speed
        // ... or above 80% / 100% depending on the generation and maker
        nearest_speed.clamp(self.config.fan_speed_floor, self.config.fan_speed_ceiling)
    }

    fn get_dwell_time(&mut self) -> bool {
        let dwell_time = Duration::from_secs(self.config.fan_dwell_time);
        if let Some(last_adjust) = self.last_adjustment_time {
            if Instant::now().duration_since(last_adjust) < dwell_time {
                return true;
            }
        }

        false
    }

    fn get_smooth_speed(&mut self, current_speed: u32, target_speed: u32) -> u32 {
        let upper_threshold = target_speed.saturating_add(self.config.hysteresis);
        let lower_threshold = target_speed.saturating_sub(self.config.hysteresis);
        let mut adjusted_speed = current_speed;

        // Calculate temperature change rate
        let temp_change_rate = (self.current_temp as f64 - self.last_temp as f64)
            / Instant::now()
                .duration_since(self.last_temp_time.unwrap())
                .as_secs_f64();

        // Adjust step size based on temperature change rate
        let base_step_size = self.config.smooth_mode_fan_step as f64;
        let step_size = if temp_change_rate > 0.0 {
            base_step_size * (1.0 + temp_change_rate * self.config.smooth_mode_incr_weight)
        } else {
            base_step_size * (1.0 - (-temp_change_rate) * self.config.smooth_mode_decr_weight)
        };

        if current_speed < lower_threshold {
            adjusted_speed = min(
                current_speed.saturating_add(step_size.round() as u32),
                upper_threshold,
            );
        } else if current_speed > upper_threshold {
            adjusted_speed = max(
                current_speed.saturating_sub(step_size.round() as u32),
                lower_threshold,
            );
        }

        self.last_temp = self.temp_average;
        adjusted_speed.clamp(self.config.fan_speed_floor, self.config.fan_speed_ceiling)
    }

    fn get_target_fan_speed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.target_fan_speed = self.select_nearest_fan_speed(self.temp_average);
        if self.config.smooth_mode {
            self.target_fan_speed =
                self.get_smooth_speed(self.current_fan_speed, self.target_fan_speed);
        }

        Ok(())
    }

    pub fn set_target_fan_speed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.get_target_fan_speed()?;

        if self.get_dwell_time() {
            return Ok(()); // Skip adjustment if within dwell time
        }

        if self.current_fan_speed != self.target_fan_speed {
            println!(
                "[{}] Veridian transitioning state: {} C => {} %A -> {}{} %T",
                helpers::get_cur_time(),
                self.temp_average,
                self.current_fan_speed,
                self.smooth_mode,
                self.target_fan_speed
            );
            commands::set_fan_speed(self.target_fan_speed)?;
            self.last_adjustment_time = Some(Instant::now());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_target_fan_speed() {
        let _config = Config::default();
        let mut manager = ThermalManager::new(&_config);
        // start at index 2
        manager.temp_average = 74;
        manager.get_target_fan_speed().unwrap();
        assert_eq!(manager.target_fan_speed, 62);
        // transition to index 4
        manager.temp_average = 89;
        manager.get_target_fan_speed().unwrap();
        assert_eq!(manager.target_fan_speed, 100);
        // transition to index 0
        manager.temp_average = 53;
        manager.get_target_fan_speed().unwrap();
        assert_eq!(manager.target_fan_speed, 46);
    }

    #[test]
    fn test_get_smooth_speed() {
        let _config = Config::default();
        let mut manager = ThermalManager::new(&_config);
        manager.last_temp_time = Some(Instant::now());
        // start at index 0
        manager.last_temp = 59;
        manager.current_temp = 59;
        manager.temp_average = 58;
        manager.current_fan_speed = 46;
        manager.target_fan_speed = 46;
        let result = manager.get_smooth_speed(manager.current_fan_speed, manager.target_fan_speed);
        assert_eq!(result, 46);
        // transition to index 3 (smoothed)
        manager.last_temp = 71;
        manager.current_temp = 73;
        manager.temp_average = 74;
        manager.current_fan_speed = 55;
        manager.target_fan_speed = 62;
        let result = manager.get_smooth_speed(manager.current_fan_speed, manager.target_fan_speed);
        assert_eq!(result, 65);
        // transition to index 4 (smoothed)
        manager.last_temp = 85;
        manager.current_temp = 86;
        manager.temp_average = 87;
        manager.current_fan_speed = 55;
        manager.target_fan_speed = 80;
        let result = manager.get_smooth_speed(manager.current_fan_speed, manager.target_fan_speed);
        assert_eq!(result, 83);
    }
}
