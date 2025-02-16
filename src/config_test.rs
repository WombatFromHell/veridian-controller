use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use crate::config;

#[test]
fn test_default_config() {
    let config = config::Config::default();
    assert_eq!(config.gpu_id, 0);
    assert_eq!(config.temp_thresholds, vec![40, 50, 60, 78, 84]);
    assert_eq!(config.fan_speeds, vec![46, 55, 62, 80, 100]);
    assert_eq!(config.fan_speed_floor, 46);
    assert_eq!(config.fan_speed_ceiling, 100);
    assert_eq!(config.sampling_window_size, 10);
    assert_eq!(config.hysteresis, 3);
    assert_eq!(config.global_delay, 2);
    assert_eq!(config.fan_dwell_time, 10);
    assert!(config.smooth_mode);
    assert_eq!(config.smooth_mode_incr_weight, 1.0);
    assert_eq!(config.smooth_mode_decr_weight, 4.0);
    assert_eq!(config.smooth_mode_max_fan_step, 5);
}

#[test]
fn test_expand_tilde() {
    // Temporarily store the original HOME value
    let original_home = env::var("HOME").ok();

    // Set a controlled home directory for testing
    env::set_var("HOME", "/home/test");

    let cases = vec![
        ("~/config.toml", "/home/test/config.toml"),
        ("~/dir/config.toml", "/home/test/dir/config.toml"),
        ("/absolute/path/config.toml", "/absolute/path/config.toml"),
        ("relative/path/config.toml", "relative/path/config.toml"),
    ];

    for (input, expected) in cases {
        let result = config::expand_tilde(input).unwrap();
        assert_eq!(result, PathBuf::from(expected));
    }

    // Restore the original HOME value
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
}

#[test]
fn test_resolve_path() {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // Test absolute path
    let abs_path = base_path.join("config.toml");
    let resolved = config::resolve_path(abs_path.to_str().unwrap()).unwrap();
    assert!(resolved.is_absolute());
    assert!(resolved.exists());

    // Test relative path
    let relative_path = "test_config.toml";
    let resolved = config::resolve_path(relative_path).unwrap();
    assert!(resolved.is_absolute());
    assert!(resolved.exists());

    // Test nested path creation
    let nested_path = base_path.join("nested/config/test.toml");
    let resolved = config::resolve_path(nested_path.to_str().unwrap()).unwrap();
    assert!(resolved.is_absolute());
    assert!(resolved.exists());
    assert!(resolved.parent().unwrap().exists());
}

#[test]
fn test_config_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    let config = config::Config::default();
    config
        .write_to_file(Some(config_path.to_str().unwrap().to_string()))
        .unwrap();

    // Read back the config and verify it matches
    let read_config = config::Config::new(Some(config_path.to_str().unwrap().to_string())).unwrap();
    assert_eq!(read_config.gpu_id, config.gpu_id);
    assert_eq!(read_config.temp_thresholds, config.temp_thresholds);
    assert_eq!(read_config.fan_speeds, config.fan_speeds);
    assert_eq!(read_config.fan_speed_floor, config.fan_speed_floor);
    assert_eq!(read_config.fan_speed_ceiling, config.fan_speed_ceiling);
}

#[test]
fn test_invalid_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("invalid_config.toml");

    // Write invalid TOML
    fs::write(&config_path, "invalid = toml [ content").unwrap();

    let result = config::Config::new(Some(config_path.to_str().unwrap().to_string()));
    assert!(matches!(result, Err(config::ConfigError::Toml(_))));
}

#[test]
fn test_mismatched_arrays() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("mismatched_config.toml");

    // Create config with mismatched arrays
    let config_content = r#"
        gpu_id = 0
        temp_thresholds = [40, 50, 60]
        fan_speeds = [46, 55]
        fan_speed_floor = 46
        fan_speed_ceiling = 100
        sampling_window_size = 10
        hysteresis = 3
        global_delay = 2
        fan_dwell_time = 10
        smooth_mode = true
        smooth_mode_incr_weight = 1.0
        smooth_mode_decr_weight = 4.0
        smooth_mode_max_fan_step = 5
    "#;

    fs::write(&config_path, config_content).unwrap();

    let result = config::Config::new(Some(config_path.to_str().unwrap().to_string()));
    assert!(matches!(
        result,
        Err(config::ConfigError::InvalidArrayFormat)
    ));
}

#[test]
fn test_load_config_from_env() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("env_config.toml");

    // Test with non-existent file (should create default)
    let config =
        config::load_config_from_env(Some(config_path.to_str().unwrap().to_string())).unwrap();
    assert_eq!(config.gpu_id, config::Config::default().gpu_id);

    // Test with existing valid file
    let config =
        config::load_config_from_env(Some(config_path.to_str().unwrap().to_string())).unwrap();
    assert_eq!(config.gpu_id, config::Config::default().gpu_id);
}
