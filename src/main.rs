use clap::Parser;
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
use std::error::Error;
use std::thread;
use std::time::Duration;

mod commands;
mod config;
mod helpers;
mod thermalmanager;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path of the config file to load
    #[arg(short, long, value_name = "PATH")]
    file: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGINT | SIGTERM => {
                    println!("Caught graceful shutdown, cleaning up...");
                    commands::set_fan_control(0).unwrap();
                    std::process::exit(1);
                }
                _ => {
                    println!("Caught signal: {:?}", signal);
                    std::process::exit(1);
                }
            }
        }
    });

    let config = config::load_config_from_env(args.file)?;

    commands::set_fan_control(1)?;
    let mut thermal_manager = thermalmanager::ThermalManager::new(&config);

    loop {
        thermal_manager.update_temperature();
        thermal_manager.calculate_target_fan_speed()?;
        thread::sleep(Duration::from_secs(config.global_delay));
    }
}
