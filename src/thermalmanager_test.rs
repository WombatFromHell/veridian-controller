use std::collections::VecDeque;

use crate::config::Config;
use crate::thermalmanager::ThermalManager;

#[test]
fn test_select_nearest_fan_speed() {
    let config = Config::default();
    let mut thermal_manager = ThermalManager::new(config.clone());

    let test_thresholds = vec![(40, 46), (50, 55), (60, 62), (74, 80), (82, 100)];
    let test_cases = vec![
        (35, 46),   // Below all thresholds, should be floor
        (40, 46),   // Exactly at first threshold
        (45, 46),   // Between first and second threshold
        (50, 55),   // Exactly at second threshold
        (55, 55),   // Between second and third threshold
        (60, 62),   // Exactly at third threshold
        (70, 62),   // Between third and fourth threshold
        (74, 80),   // Exactly at fourth threshold
        (80, 80),   // Between fourth and fifth threshold
        (82, 100),  // Exactly at fifth threshold
        (90, 100),  // Above all thresholds, should be ceiling
        (105, 100), // Above all thresholds, should be ceiling
    ];

    for (temp, expected_speed) in test_cases {
        thermal_manager.current_temp = temp;
        let actual_speed = thermal_manager.select_nearest_fan_speed(test_thresholds.clone());
        assert_eq!(
            actual_speed, expected_speed,
            "For temp {}, expected speed {}, but got {}",
            temp, expected_speed, actual_speed
        );
    }

    // Test with empty thresholds (should return floor):
    let empty_thresholds: Vec<(u64, u64)> = Vec::new();
    thermal_manager.current_temp = 50; // Doesn't matter what temp is with no thresholds
    let actual_speed = thermal_manager.select_nearest_fan_speed(empty_thresholds);
    assert_eq!(
        actual_speed,
        config.clone().fan_speed_floor,
        "With empty thresholds, should return floor"
    );

    // Test with thresholds where speed is lower than floor (should clamp to floor):
    let low_speed_thresholds = vec![(50, 20)];
    thermal_manager.current_temp = 50;
    let actual_speed = thermal_manager.select_nearest_fan_speed(low_speed_thresholds);
    assert_eq!(
        actual_speed,
        config.clone().fan_speed_floor,
        "Speed below floor should clamp"
    );

    // Test with thresholds where speed is higher than ceiling (should clamp to ceiling):
    let high_speed_thresholds = vec![(50, 120)];
    thermal_manager.current_temp = 50;
    let actual_speed = thermal_manager.select_nearest_fan_speed(high_speed_thresholds);
    assert_eq!(
        actual_speed,
        config.clone().fan_speed_ceiling,
        "Speed above ceiling should clamp"
    );
}

#[test]
fn test_calculate_wma() {
    let config = Config::default();
    let mut thermal_manager = ThermalManager::new(config);

    // Test with varying temperatures
    thermal_manager.samples = VecDeque::from(vec![40, 50, 60, 70, 80]);
    assert_eq!(thermal_manager.calculate_wma(), 57);

    // Test with constant temperature
    thermal_manager.samples = VecDeque::from(vec![40, 40, 40, 40, 40]);
    assert_eq!(thermal_manager.calculate_wma(), 40);

    // Test with high constant temperature
    thermal_manager.samples = VecDeque::from(vec![80, 80, 80, 80, 80]);
    assert_eq!(thermal_manager.calculate_wma(), 80);

    // Test with increasing temperatures
    thermal_manager.samples = VecDeque::from(vec![45, 55, 65, 75, 85]);
    assert_eq!(thermal_manager.calculate_wma(), 62);

    // Test with small variations
    thermal_manager.samples = VecDeque::from(vec![44, 46, 50, 54, 56]);
    assert_eq!(thermal_manager.calculate_wma(), 49);

    // Test with small increasing variations
    thermal_manager.samples = VecDeque::from(vec![40, 42, 44, 46, 48]);
    assert_eq!(thermal_manager.calculate_wma(), 43);

    // Test with small decreasing variations
    thermal_manager.samples = VecDeque::from(vec![52, 54, 56, 58, 60]);
    assert_eq!(thermal_manager.calculate_wma(), 55);
}

#[test]
fn test_get_smooth_speed() {
    let config = Config::default();
    let mut thermal_manager = ThermalManager::new(config);
    let thresholds = thermal_manager.generate_thresholds_and_speeds();

    // Test cases: (current_temp, current_fan_speed, expected_result)
    let test_cases = vec![
        (39, 0, 46),   // Test speed floor
        (55, 60, 59),  // Increasing temperature
        (57, 60, 60),  // Test relative stability
        (60, 65, 62),  // At upper threshold
        (82, 90, 100), // Test speed ceiling
        (94, 90, 100), // Beyond max threshold
        (62, 50, 60),  // Max step limit (increase)
        (42, 60, 50),  // Max step limit (decrease)
        (76, 46, 56),  // Beyond max step limit (increase)
        (32, 80, 70),  // Beyond max step limit (decrease)
    ];

    for (temp, speed, expected) in test_cases {
        thermal_manager.current_temp = temp;
        thermal_manager.current_fan_speed = speed;
        assert_eq!(
            thermal_manager.get_smooth_speed(&thresholds),
            expected,
            "Failed at temp: {}, current fan speed: {}, expected: {}",
            temp,
            speed,
            expected
        );
    }
}
