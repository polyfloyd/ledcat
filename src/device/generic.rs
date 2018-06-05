use std::io;
use device::*;
use clap;


pub struct Generic { }

impl Device for Generic {
    fn color_correction(&self) -> Correction {
        Correction::none()
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        let buf: Vec<u8> = pixels.iter()
            .flat_map(|pix| vec![pix.r, pix.g, pix.b])
            .collect();
        writer.write_all(&buf)?;
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("generic")
        .about("Output data as RGB24")
}

pub fn from_command(_: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    Ok(FromCommand::Device(Box::new(Generic {})))
}
