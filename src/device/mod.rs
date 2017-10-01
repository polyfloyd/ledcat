use std::io;
use std::ops::Deref;
use clap;
use color::*;
use driver::*;

pub mod apa102;
pub mod generic;
pub mod hexws2811;
pub mod hub75;
pub mod lpd8806;


/// An output represents the device that is used as output.
///
/// It is also possible to compose an output from a Device and an `io::Write` to allow reuse of
/// driver code.
pub trait Output {
    fn color_correction(&self) -> Correction;

    fn output_frame(&mut self, &[Pixel]) -> io::Result<()>;
}

impl<D, W> Output for (D, W)
    where D: Device,
          W: io::Write {
    fn color_correction(&self) -> Correction {
        self.0.color_correction()
    }

    fn output_frame(&mut self, frame: &[Pixel]) -> io::Result<()> {
        self.0.write_frame(&mut self.1, frame)
    }
}


/// The Device is half of an output system and represents the wire format of some physical device.
///
/// The other half of the output is formed by the driver modules which handle the actual IO to the
/// device.
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

impl<T> Device for Box<T>
    where T: Device + ?Sized {
    fn color_correction(&self) -> Correction {
        self.deref().color_correction()
    }

    fn write_frame(&self, out: &mut io::Write, frame: &[Pixel]) -> io::Result<()> {
        self.deref().write_frame(out, frame)
    }
}

pub fn devices<'a, 'b>() -> Vec<(clap::App<'a, 'b>, fn(&clap::ArgMatches) -> Box<Device>)> {
    vec![(apa102::command(), apa102::from_command),
         (generic::command(), generic::from_command),
         (hexws2811::command(), hexws2811::from_command),
         (lpd8806::command(), lpd8806::from_command)]
}
