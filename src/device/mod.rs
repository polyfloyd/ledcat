use std::io;
use clap;
use color::*;

pub mod apa102;
pub mod lpd8806;
pub mod raw;

#[derive(Clone)]
pub enum FirstBit { LSB, MSB }

pub trait Device {
    fn clock_phase(&self) -> u8;
    fn clock_polarity(&self) -> u8;
    fn first_bit(&self) -> FirstBit;
    fn color_correction(&self) -> Correction;
    fn write_frame(&self, &mut io::Write, &[Pixel]) -> io::Result<()>;
}

pub fn devices<'a, 'b>() -> Vec<(clap::App<'a, 'b>, fn(&clap::ArgMatches) -> Box<Device>)> {
    vec![
        (apa102::command(), apa102::from_command),
        (lpd8806::command(), lpd8806::from_command),
        (raw::command(), raw::from_command),
    ]
}
