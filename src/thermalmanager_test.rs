use crate::config::Config;
use crate::thermalmanager::ThermalManager;
use std::time::Instant;

#[test]
fn test_get_target_fan_speed() {
    let config = Config {
        smooth_mode: false,
        ..Config::default()
    };
    let mut manager = ThermalManager::new(config);

    // Test case 1: Temperature below first threshold (40°C)
    manager.current_temp = 38;
    manager.temp_average = 38;
    manager.get_target_fan_speed();
    assert_eq!(
        manager.target_fan_speed, 46,
        "Should be at floor speed for temperature below first threshold"
    );

    manager.current_temp = 53; // 50°C + 3°C hysteresis
    manager.temp_average = 53;
    manager.get_target_fan_speed();
    assert_eq!(
        manager.target_fan_speed, 55,
        "Should be at second fan speed tier"
    );

    manager.current_temp = 81; // 78°C + 3°C hysteresis
    manager.temp_average = 81;
    manager.get_target_fan_speed();
    assert_eq!(
        manager.target_fan_speed, 80,
        "Should be at fourth fan speed tier"
    );

    manager.current_temp = 87; // 84°C + 3°C hysteresis
    manager.temp_average = 87;
    manager.get_target_fan_speed();
    assert_eq!(manager.target_fan_speed, 100, "Should be at maximum speed");
}

#[test]
fn test_get_smooth_speed() {
    let config = Config::default();
    let mut manager = ThermalManager::new(config);
    manager.last_temp_time = Some(Instant::now());

    manager.current_temp = 65;
    manager.temp_average = 65;
    manager.current_fan_speed = 40;
    let thresholds = manager.generate_thresholds_and_speeds();
    let result = manager.get_smooth_speed(thresholds.clone());
    // At 65°C, we expect the speed to be between the two threshold values (30-40)
    assert!(
        (40..=50).contains(&result),
        "Should set appropriate fan speed for temperature between thresholds: got {}",
        result
    );

    manager.current_temp = 75;
    manager.temp_average = 70;
    manager.current_fan_speed = 50;
    let result = manager.get_smooth_speed(thresholds.clone());
    // Speed should increase but be limited by max_fan_step (10)
    assert!(
        result >= manager.current_fan_speed
            && result <= manager.current_fan_speed + manager.config.smooth_mode_max_fan_step,
        "Should increase fan speed within max_fan_step limit: got {}",
        result
    );

    // Test case 3: Very high temperature
    manager.current_temp = 90;
    manager.temp_average = 88;
    manager.current_fan_speed = 90;
    let result = manager.get_smooth_speed(thresholds);
    // Should hit ceiling for very high temperature
    assert!(
        result >= 90,
        "Should maintain high speed for very high temperature: got {}",
        result
    );
}
