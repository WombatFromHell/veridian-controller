use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Result};
use std::process;

const LOCK_FILE_PATH: &str = "/tmp/veridian-controller.lock";

pub struct LockGuard {
    lock_file: File
}

fn create_lock_file() -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(LOCK_FILE_PATH)
        .map_err(|err| match err.kind() {
            ErrorKind::AlreadyExists => Error::new(
                ErrorKind::AlreadyExists,
                "Another instance of the program is already running.",
            ),
            _ => err,
        })
}

fn remove_lock_file(lock_file: &File) -> Result<()> {
    lock_file.sync_all()?;
    std::fs::remove_file(LOCK_FILE_PATH)?;
    Ok(())
}

pub fn acquire_lock() -> Result<LockGuard> {
    let lock_file = create_lock_file()?;
    Ok(LockGuard {
        lock_file,
    })
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Err(err) = remove_lock_file(&self.lock_file) {
            eprintln!("Error removing lock file: {}", err);
            process::exit(1);
        }
    }
}
