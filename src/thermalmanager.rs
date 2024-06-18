use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::commands;
use crate::config::Config;
use crate::helpers;

pub struct ThermalManager<'a> {
    samples: VecDeque<u64>,
    config: &'a Config,
    temp_average: u64,
    current_temp: u64,
    last_adjustment_time: Option<Instant>,
    last_temp_time: Option<Instant>,
    current_fan_speed: u64,
    target_fan_speed: u64,
    smooth_mode: &'a str,
}

impl<'a> ThermalManager<'a> {
    pub fn new(config: &'a Config) -> Self {
        ThermalManager {
            samples: VecDeque::with_capacity(config.sampling_window_size),
            config,
            temp_average: 0,
            current_temp: 0,
            last_adjustment_time: None,
            last_temp_time: None,
            current_fan_speed: 0,
            target_fan_speed: config.fan_speed_floor as u64,
            smooth_mode: if config.smooth_mode { "~" } else { "" },
        }
    }

    pub fn update_temperature(&mut self) {
        self.current_temp = commands::get_gpu_temp();
        self.last_temp_time = Some(Instant::now());
        self.current_fan_speed = commands::get_fan_speed() as u64;
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

        // rearrange into descending order
        _temps
            .into_iter()
            .zip(_speeds.into_iter())
            .rev()
            .collect::<Vec<(u64, u64)>>()
    }

    fn calculate_wma(&mut self) -> u64 {
        let mut temp_average: f64 = 0.0;
        let mut weight_sum: f64 = 0.0;

        for (i, temp) in self.samples.iter().enumerate() {
            let weight = (self.config.sampling_window_size - i) as f64;
            temp_average += weight * (*temp as f64);
            weight_sum += weight;
        }

        (temp_average / weight_sum) as u64
    }

    fn select_nearest_fan_speed(&mut self, thresholds: Vec<(u64, u64)>) -> u64 {
        // create an array of descending tuples and return matching speed directly
        let mut nearest_speed = self.config.fan_speed_floor;

        for (thresh, speed) in thresholds.iter() {
            let hyst_hi = thresh.saturating_add(self.config.hysteresis);
            // prefer the higher threshold to reduce overheating
            if self.current_temp >= hyst_hi {
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
            let from_last_adjust = Instant::now().duration_since(last_adjust);
            if from_last_adjust < dwell_time {
                return true;
            }
        }

        false
    }

    fn get_smooth_speed(&mut self, thresholds: Vec<(u64, u64)>) -> u64 {
        let base_speed = self.select_nearest_fan_speed(thresholds);

        let fan_speed_range = (self.config.fan_speed_ceiling - self.config.fan_speed_floor) as f64;

        let target_speed = base_speed as f64
            + (self.current_temp as f64 - base_speed as f64) * fan_speed_range
                / self.config.fan_speed_ceiling as f64;

        let incr_weight = self.config.smooth_mode_incr_weight;
        let decr_weight = self.config.smooth_mode_decr_weight;

        let temp_diff = self.current_temp as f64 - self.temp_average as f64;
        let hysteresis_range = self.config.hysteresis as f64;

        let temp_diff_weighted = if temp_diff.abs() < hysteresis_range {
            0.0
        } else {
            temp_diff * fan_speed_range * (incr_weight - decr_weight) / 2.0
        };

        let weighted_average =
            incr_weight * self.current_temp as f64 + decr_weight * self.temp_average as f64;
        let _smooth_speed = target_speed + temp_diff_weighted / weighted_average;

        let output_diff = _smooth_speed - self.current_fan_speed as f64;
        let abs_output_diff = output_diff.max(-output_diff);
        let max_speed_change = self.config.smooth_mode_max_fan_step;

        let smooth_speed = if abs_output_diff < self.config.hysteresis as f64 {
            self.current_fan_speed
        } else {
            // limit the max speed change per adjustment period
            let limit_change = output_diff
                .max(-(max_speed_change as i64) as f64)
                .min(max_speed_change as f64);
            (self.current_fan_speed as f64 + limit_change) as u64
        };

        smooth_speed.clamp(self.config.fan_speed_floor, self.config.fan_speed_ceiling)
    }

    fn get_target_fan_speed(&mut self) -> u64 {
        let thresholds = self.generate_thresholds_and_speeds();

        if self.config.smooth_mode {
            self.target_fan_speed = self.get_smooth_speed(thresholds.clone());
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
        manager.get_target_fan_speed();
        assert_eq!(manager.target_fan_speed, 62);
        // transition to index 4
        manager.temp_average = 89;
        manager.get_target_fan_speed();
        assert_eq!(manager.target_fan_speed, 100);
        // transition to index 0
        manager.temp_average = 53;
        manager.get_target_fan_speed();
        assert_eq!(manager.target_fan_speed, 46);
    }

    #[test]
    fn test_get_smooth_speed() {
        let _config = Config::default();
        let mut manager = ThermalManager::new(&_config);
        let thresholds = manager.generate_thresholds_and_speeds();
        manager.last_temp_time = Some(Instant::now());
        // start at index 0
        manager.current_temp = 59;
        manager.temp_average = 58;
        manager.current_fan_speed = 46;
        manager.target_fan_speed = 46;
        let result = manager.get_smooth_speed(thresholds.clone());
        assert_eq!(result, 46);
        // transition to index 3 (smoothed)
        manager.current_temp = 73;
        manager.temp_average = 74;
        manager.current_fan_speed = 55;
        manager.target_fan_speed = 62;
        let result = manager.get_smooth_speed(thresholds.clone());
        assert_eq!(result, 65);
        // transition to index 4 (smoothed)
        manager.current_temp = 86;
        manager.temp_average = 87;
        manager.current_fan_speed = 55;
        manager.target_fan_speed = 80;
        let result = manager.get_smooth_speed(thresholds.clone());
        assert_eq!(result, 83);
    }
}
