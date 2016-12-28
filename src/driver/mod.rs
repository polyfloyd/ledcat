use std::path;

pub mod spidev;

const driver_detectors: &'static [(&'static str, fn(&path::PathBuf) -> bool)] = &[
    ("spidev", spidev::is_spidev),
];

pub fn detect(file: &path::PathBuf) -> Option<String> {
    for dr in driver_detectors {
        if dr.1(file) {
            return Some(dr.0.to_string());
        }
    }
    None
}
