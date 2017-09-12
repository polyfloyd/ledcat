use std::io;
use device::*;
use clap;


pub struct Generic { }

impl Device for Generic {
    fn color_correction(&self) -> Correction {
        Correction::none()
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        for pix in pixels.iter() {
            writer.write_all(&[pix.r, pix.g, pix.b])?;
        }
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("generic")
        .about("Output data as RGB24")
}

pub fn from_command(_: &clap::ArgMatches) -> Box<Device> {
    Box::new(Generic {})
}
