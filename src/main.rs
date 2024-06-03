use clap::Parser;
use signal_hook::flag;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod commands;
mod config;
mod filelock;
mod helpers;
mod thermalmanager;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]

pub struct Args {
    /// Path of the config file to load
    #[arg(short, long, value_name = "PATH")]
    file: Option<String>,
}

fn cleanup() -> Result<(), Box<dyn Error>> {
    println!("Attempting to gracefully shutdown...");
    commands::set_fan_control(0)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    match filelock::acquire_lock() {
        // ensure only one copy of the program is running at a time
        Ok(lock) => {
            let terminate = Arc::new(AtomicBool::new(false));
            let args = Args::parse();

            // register some common termination signals for use with Ctrl+C and SystemD
            flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminate))?;
            flag::register(signal_hook::consts::SIGABRT, Arc::clone(&terminate))?;
            flag::register(signal_hook::consts::SIGINT, Arc::clone(&terminate))?;

            let config = config::load_config_from_env(args.file)?;
            let mut thermal_manager = thermalmanager::ThermalManager::new(&config);

            while !terminate.load(Ordering::Relaxed) {
                thermal_manager.update_temperature();
                thermal_manager.calculate_target_fan_speed()?;
                thread::sleep(Duration::from_secs(config.global_delay));
            }

            cleanup()?;
            drop(lock);
        }
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        }
    }

    Ok(())
}
