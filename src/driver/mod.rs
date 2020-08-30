use std::error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod serial;
pub mod spidev;

#[allow(clippy::type_complexity)]
const DRIVER_DETECTORS: &[(&str, fn(&Path) -> bool)] =
    &[("serial", serial::is_serial), ("spidev", spidev::is_spidev)];

pub fn detect(file: impl AsRef<Path>) -> Option<String> {
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

fn read_link_recursive(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    match fs::read_link(path.as_ref()) {
        Ok(path) => read_link_recursive(path),
        Err(err) => {
            if err.raw_os_error() == Some(22) {
                Ok(path.as_ref().to_path_buf())
            } else {
                Err(err)
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {
    DeviceNotSupported,
    Io(io::Error),
    Nix(nix::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<nix::Error> for Error {
    fn from(err: nix::Error) -> Self {
        Error::Nix(err)
    }
}
