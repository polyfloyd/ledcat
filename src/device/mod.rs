use std::io;
use std::ops::Deref;
use clap;
use color::*;
use driver::*;
use geometry::*;

pub mod apa102;
pub mod generic;
pub mod hexws2811;
pub mod hub75;
pub mod lpd8806;
pub mod simulator;
pub mod ws2812;


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


pub struct GlobalArgs {
    pub dimensions: Option<Dimensions>,
}

impl GlobalArgs {
    pub fn dimensions(&self) -> io::Result<Dimensions> {
        self.dimensions.ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Please set the frame size"))
    }

    pub fn dimensions_2d(&self) -> io::Result<(usize, usize)> {
        match self.dimensions()? {
            Dimensions::One(_) => {
                Err(io::Error::new(io::ErrorKind::Other, "This device requires 2D geometry"))
            },
            Dimensions::Two(w, h) => Ok((w, h)),
        }
    }
}

/// Device implemetations are expected to be accompanied by a function that constructs and
/// configures a new instance from a set of command line arguments.
pub enum FromCommand {
    /// A device was constructed, but no driver was configured/implemented.
    Device(Box<Device>),
    /// An output was constructed, no actions to find a driver are needed.
    Output(Box<Output>),
    /// A subcommand was handled, terminate the program without performing IO.
    SubcommandHandled,
}

pub type FromCommandFn = fn(&clap::ArgMatches, &GlobalArgs) -> io::Result<FromCommand>;

pub fn devices<'a, 'b>() -> Vec<(clap::App<'a, 'b>, FromCommandFn)> {
    vec![
        (apa102::command(), apa102::from_command),
        (artnet::command(), artnet::from_command),
        (generic::command(), generic::from_command),
        (hexws2811::command(), hexws2811::from_command),
        (hub75::command(), hub75::from_command),
        (lpd8806::command(), lpd8806::from_command),
        (simulator::command(), simulator::from_command),
        (ws2812::command(), ws2812::from_command),
    ]
}
