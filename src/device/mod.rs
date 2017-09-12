use std::io;
use clap;
use color::*;
use driver::*;

pub mod apa102;
pub mod generic;
pub mod hexws2811;
pub mod lpd8806;


pub trait Device {
    fn color_correction(&self) -> Correction;
    fn write_frame(&self, &mut io::Write, &[Pixel]) -> io::Result<()>;

    fn spidev_config(&self) -> Option<spidev::Config> {
        None
    }

    fn written_frame_size(&self, num_pixels: usize) -> usize {
        let mut buf = Vec::new();
        let dummy_frame: Vec<Pixel> = (0..num_pixels)
            .map(|_| Pixel { r: 0, g: 0, b: 0 })
            .collect();
        self.write_frame(&mut buf, dummy_frame.as_slice()).unwrap();
        buf.len()
    }
}

pub fn devices<'a, 'b>() -> Vec<(clap::App<'a, 'b>, fn(&clap::ArgMatches) -> Box<Device>)> {
    vec![(apa102::command(), apa102::from_command),
         (generic::command(), generic::from_command),
         (hexws2811::command(), hexws2811::from_command),
         (lpd8806::command(), lpd8806::from_command)]
}
