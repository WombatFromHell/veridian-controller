use clap::Parser;
use std::error::Error;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

mod commands;
mod config;
mod filelock;
mod thermalmanager;

#[cfg(test)]
mod thermalmanager_test;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path of the config file to load
    #[arg(short, long, value_name = "PATH")]
    file: Option<String>,
}

fn cleanup(gpu_id: &u8) -> Result<(), Box<dyn Error>> {
    println!("Attempting to gracefully shutdown...");
    commands::set_fan_control(gpu_id, 0)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let terminate = Arc::new(AtomicBool::new(false));
    filelock::acquire_lock()?;

    let config = Arc::new(RwLock::new(config::load_config_from_env(args.file)?));
    let config_guard = config.read().unwrap();
    let gpu_id = config_guard.gpu_id;
    let global_delay = config_guard.global_delay;

    // register common signals representing 'shutdown'
    for sig in &[
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGABRT,
    ] {
        signal_hook::flag::register(*sig, Arc::clone(&terminate))?;
    }

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        eprintln!("Panic occurred: {:?}", panic_info);
        default_panic(panic_info);
        // try to gracefully shutdown when panicing
        if let Err(e) = cleanup(&gpu_id) {
            eprintln!("Error during cleanup: {:?}", e);
        }
        std::process::exit(1);
    }));

    // preemptively lock fan control for our use
    commands::set_fan_control(&gpu_id, 1)?;

    let thermal_manager = {
        let thermal_guard = match config.read() {
            Ok(thermal_guard) => thermal_guard,
            Err(err) => {
                eprintln!("Thermal config lock poisoned: {}", err);
                std::process::exit(1);
            }
        };

        Arc::new(RwLock::new(thermalmanager::ThermalManager::new(
            thermal_guard.clone(),
        )))
    };

    let thermal_thread = {
        let terminate = Arc::clone(&terminate);
        let thermal_manager_lock = Arc::clone(&thermal_manager);

        thread::spawn(move || {
            while !terminate.load(Ordering::SeqCst) {
                if let Err(e) = catch_unwind(|| {
                    if let Ok(mut manager) = thermal_manager_lock.write() {
                        manager.update_temperature();
                        if let Err(e) = manager.set_target_fan_speed() {
                            eprintln!("Failed to set fan speed: {:?}", e);
                            std::process::exit(1);
                        }
                    }
                }) {
                    eprintln!("Error in thermal thread: {:?}", e);
                    break;
                }

                // update the temperature/fan-speed every X seconds
                thread::sleep(Duration::from_secs(global_delay));
            }
        })
    };

    // watch for exit signal
    while !terminate.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    // try to gracefully shutdown
    cleanup(&gpu_id)?;
    if let Err(e) = thermal_thread.join() {
        eprintln!("Thermal thread panicked: {:?}", e);
    }

    Ok(())
}
