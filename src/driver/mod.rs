use std::fs;
use std::io;
use std::path;
use nix;

pub mod artnet;
pub mod serial;
pub mod spidev;


const DRIVER_DETECTORS: &[(&str, fn(&path::PathBuf) -> bool)] = &[
    ("serial", serial::is_serial),
    ("spidev", spidev::is_spidev),
];

pub fn detect(file: &path::PathBuf) -> Option<String> {
    let real_file = match read_link_recursive(file) {
        Ok(p) => p,
        Err(_) => return None,
    };
    for dr in DRIVER_DETECTORS {
        if dr.1(&real_file) {
            return Some(dr.0.to_string());
        }
    }
    None
}

fn read_link_recursive(path: &path::PathBuf) -> io::Result<path::PathBuf> {
    match fs::read_link(&path) {
        Ok(path) => read_link_recursive(&path),
        Err(err) => {
            if err.raw_os_error() == Some(22) {
                Ok(path.to_path_buf())
            } else {
                Err(err)
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    DeviceNotSupported,
    Io(io::Error),
    Nix(nix::Error),
}
