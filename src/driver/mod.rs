use nix;
use std::fs;
use std::io;
use std::path;

pub mod artnet;
pub mod serial;
pub mod spidev;

const DRIVER_DETECTORS: &[(&str, fn(&path::Path) -> bool)] =
    &[("serial", serial::is_serial), ("spidev", spidev::is_spidev)];

pub fn detect<P: AsRef<path::Path>>(file: P) -> Option<String> {
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

fn read_link_recursive<P: AsRef<path::Path>>(path: P) -> io::Result<path::PathBuf> {
    match fs::read_link(&path) {
        Ok(path) => read_link_recursive(&path),
        Err(err) => {
            if err.raw_os_error() == Some(22) {
                Ok(path.as_ref().to_path_buf())
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
