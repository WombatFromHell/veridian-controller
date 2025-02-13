use nix::unistd::{getuid, Uid};
use std::process::{Command, Stdio};

pub fn get_gpu_temp(gpu_id: &u8) -> u64 {
    let output = Command::new("nvidia-smi")
        .args([
            format!("--id={}", gpu_id).as_str(),
            "--query-gpu=temperature.gpu",
            "--format=csv,noheader",
        ])
        .output()
        .expect("Failed to execute nvidia-smi");

    let temp_str = String::from_utf8_lossy(&output.stdout);

    temp_str.trim().parse::<u64>().unwrap_or(0).clamp(0, 200)
}

pub fn get_fan_speed(gpu_id: &u8) -> u64 {
    let output = Command::new("nvidia-smi")
        .args([
            format!("--id={}", gpu_id).as_str(),
            "--query-gpu=fan.speed",
            "--format=csv,noheader",
        ])
        .output()
        .expect("Failed to execute nvidia-smi");

    let _speed_str = String::from_utf8_lossy(&output.stdout);
    let speed_str = _speed_str.trim().replace(" %", "");

    speed_str.parse::<u64>().unwrap_or(0).clamp(0, 100)
}

pub fn set_fan_control(gpu_id: &u8, mode: u8) -> Result<(), Box<dyn std::error::Error>> {
    let is_root = Uid::is_root(getuid());

    let mut command = if is_root {
        Command::new("nvidia-settings")
    } else {
        let mut cmd = Command::new("sudo");
        cmd.arg("nvidia-settings");
        cmd
    };

    let output = command
        .args([
            "-c",
            gpu_id.to_string().as_str(),
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

pub fn set_fan_speed(gpu_id: &u8, speed: u64) -> Result<(), Box<dyn std::error::Error>> {
    let is_root = Uid::is_root(getuid());

    let mut command = if is_root {
        Command::new("nvidia-settings")
    } else {
        Command::new("sudo")
    };

    let output = command
        .args([
            "-c",
            gpu_id.to_string().as_str(),
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
