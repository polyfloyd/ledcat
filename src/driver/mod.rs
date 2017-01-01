use std::path;

pub mod artnet;
pub mod spidev;

const DRIVER_DETECTORS: &'static [(&'static str, fn(&path::PathBuf) -> bool)] = &[
    ("spidev", spidev::is_spidev),
];

pub fn detect(file: &path::PathBuf) -> Option<String> {
    for dr in DRIVER_DETECTORS {
        if dr.1(file) {
            return Some(dr.0.to_string());
        }
    }
    None
}
