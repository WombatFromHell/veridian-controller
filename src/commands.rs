use std::process::{Command, Stdio};

pub fn get_gpu_temp() -> u64 {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=temperature.gpu", "--format=csv,noheader"])
        .output()
        .expect("Failed to execute nvidia-smi");

    let temp_str = String::from_utf8_lossy(&output.stdout);

    temp_str.trim().parse::<u64>().unwrap_or(0).clamp(0, 200)
}

pub fn get_fan_speed() -> u64 {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=fan.speed", "--format=csv,noheader"])
        .output()
        .expect("Failed to execute nvidia-smi");

    let _speed_str = String::from_utf8_lossy(&output.stdout);
    let speed_str = _speed_str.trim().replace(" %", "");

    speed_str.parse::<u64>().unwrap_or(0).clamp(0, 100)
}

pub fn set_fan_control(mode: u8) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("sudo")
        .args([
            "nvidia-settings",
            "-c",
            "0",
            "-a",
            format!("GPUFanControlState={}", mode).as_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to execute nvidia-settings: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

pub fn set_fan_speed(speed: u64) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("sudo")
        .args([
            "nvidia-settings",
            "-c",
            "0",
            "-a",
            "GPUFanControlState=1",
            "-a",
            &format!("GPUTargetFanSpeed={}", speed),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to execute nvidia-settings: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}
