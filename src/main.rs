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
mod helpers;
mod thermalmanager;

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

    let config = Box::leak(Box::new(config::load_config_from_env(args.file)?));
    let gpu_id = config.gpu_id;
    let global_delay = config.global_delay;

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

    let thermal_manager = Arc::new(RwLock::new(thermalmanager::ThermalManager::new(config)));
    let thermal_thread = {
        let terminate = Arc::clone(&terminate);
        let thermal_manager_lock = Arc::clone(&thermal_manager);

        thread::spawn(move || {
            while !terminate.load(Ordering::SeqCst) {
                let result = catch_unwind(|| {
                    if let Ok(mut manager) = thermal_manager_lock.write() {
                        manager.update_temperature();
                        if let Err(e) = manager.set_target_fan_speed() {
                            eprintln!("Failed to set fan speed: {:?}", e);
                        }
                    }
                });

                if let Err(e) = result {
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
