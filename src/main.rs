use clap::Parser;
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
    let terminate = Arc::new(AtomicBool::new(false));
    let terminate_clone = Arc::clone(&terminate);

    let _ = filelock::acquire_lock()?;

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
        // try to ensure cleanup is called
        cleanup().unwrap();
        std::process::exit(1);
    }));

    thread::spawn(move || {
        while !terminate_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
        }
        eprintln!("Termination signal received");
        if let Err(e) = cleanup() {
            eprintln!("Error during cleanup: {:?}", e);
        }
        std::process::exit(0);
    });

    let result = std::panic::catch_unwind(|| {
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

        Ok(())
    });

    match result {
        Ok(inner_result) => inner_result,
        Err(_) => {
            eprintln!("Program panicked");
            Err("Program panicked".into())
        }
    }
}
