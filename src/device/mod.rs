use crate::color::*;
use crate::geometry::*;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

pub mod artnet;
pub mod fluxled;
pub mod generic;
pub mod hexws2811;
pub mod hub75;
#[cfg(feature = "rpi-led-matrix")]
pub mod rpi_led_matrix;
pub mod simulator;

/// An output represents the device that is used as output.
///
/// It is also possible to compose an output from a Device and an `io::Write` to allow reuse of
/// driver code.
pub trait Output: Send {
    fn output_frame(&mut self, frame: &[Pixel]) -> io::Result<()>;

    fn color_correction(&self) -> Correction {
        Correction::none()
    }
}

impl<D, W> Output for (D, W)
where
    D: Device + Send,
    W: io::Write + Send,
{
    fn output_frame(&mut self, frame: &[Pixel]) -> io::Result<()> {
        self.0.write_frame(&mut self.1, frame)
    }

    fn color_correction(&self) -> Correction {
        self.0.color_correction()
    }
}

impl Output for Box<dyn Output> {
    fn output_frame(&mut self, pixels: &[Pixel]) -> io::Result<()> {
        self.deref_mut().output_frame(pixels)
    }

    fn color_correction(&self) -> Correction {
        self.deref().color_correction()
    }
}

/// The Device is half of an output system and represents the wire format of some physical device.
///
/// The other half of the output is formed by the driver modules which handle the actual IO to the
/// device.
pub trait Device: Send {
    fn write_frame(&self, w: &mut dyn io::Write, frame: &[Pixel]) -> io::Result<()>;

    fn color_correction(&self) -> Correction {
        Correction::none()
    }
}

impl<T> Device for Box<T>
where
    T: Device + ?Sized,
{
    fn color_correction(&self) -> Correction {
        self.deref().color_correction()
    }

    fn write_frame(&self, out: &mut dyn io::Write, frame: &[Pixel]) -> io::Result<()> {
        self.deref().write_frame(out, frame)
    }
}

pub struct GlobalArgs {
    pub output_file: PathBuf,
    pub dimensions: Option<Dimensions>,
}

impl GlobalArgs {
    pub fn dimensions(&self) -> io::Result<Dimensions> {
        self.dimensions.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "Please set the frame size with --geometry",
            )
        })
    }
}

/// Device implemetations are expected to be accompanied by a function that constructs and
/// configures a new instance from a set of command line arguments.
pub enum FromCommand {
    /// A device was constructed, but no driver was configured/implemented.
    Device(Box<dyn Device>),
    /// An output was constructed, no actions to find a driver are needed.
    Output(Box<dyn Output>),
    /// A subcommand was handled, terminate the program without performing IO.
    SubcommandHandled,
}

pub type FromCommandFn = fn(&clap::ArgMatches, &GlobalArgs) -> io::Result<FromCommand>;

pub fn devices() -> Vec<(clap::Command, FromCommandFn)> {
    vec![
        (artnet::command(), artnet::from_command),
        (fluxled::command(), fluxled::from_command),
        (generic::command(), generic::from_command),
        (hexws2811::command(), hexws2811::from_command),
        (hub75::command(), hub75::from_command),
        #[cfg(feature = "rpi-led-matrix")]
        (rpi_led_matrix::command(), rpi_led_matrix::from_command),
        (simulator::command(), simulator::from_command),
    ]
}
