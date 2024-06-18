use clap::Parser;
use signal_hook::flag;
use std::error::Error;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    let args = Args::parse();

    // ensure only one copy of the program is running at a time
    match filelock::acquire_lock() {
        Ok(_) => {
            // try to disable fan control if we panic for whatever reason
            std::panic::set_hook(Box::new(|panic_info| {
                // capture the panic cause and try to pass it along
                // capture the panic cause and try to pass it along
                if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
                    eprintln!("Panic: {}", payload);
                } else if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
                    eprintln!("Panic: {}", payload);
                } else {
                    eprintln!("Panic: {:?}", panic_info.payload());
                }
            }));

            let terminate = Arc::new(AtomicBool::new(false));

            // register some common termination signals for use with Ctrl+C and SystemD
            flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminate))?;
            flag::register(signal_hook::consts::SIGABRT, Arc::clone(&terminate))?;
            flag::register(signal_hook::consts::SIGINT, Arc::clone(&terminate))?;

            commands::set_fan_control(1)?;
            let shared_config = Arc::new(Mutex::new(config::load_config_from_env(args.file)?));

            let thermal_thread = thread::spawn(move || {
                let config = shared_config.lock().unwrap();
                let thermal_manager =
                    Arc::new(Mutex::new(thermalmanager::ThermalManager::new(&config)));

                while !terminate.load(Ordering::Relaxed) {
                    let result = catch_unwind(|| {
                        thermal_manager.lock().unwrap().update_temperature();
                        thermal_manager
                            .lock()
                            .unwrap()
                            .set_target_fan_speed()
                            .unwrap();
                    });
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error: {:?}", e);
                            std::process::exit(1);
                        }
                    }

                    thread::sleep(Duration::from_secs(config.global_delay));
                }
            });

            let _ = thermal_thread.join().unwrap_or_else(|err| {
                eprintln!("Error in thread: {:?}", err);
            });

            cleanup()?;
        }
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        }
    }

    Ok(())
}
