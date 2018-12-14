use crate::device::*;
use clap;
use std::io;

pub struct HexWS2811 {}

impl Device for HexWS2811 {
    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        for pix in pixels.iter().rev() {
            writer.write_all(&[
                ((u16::from(pix.g) * 256) & 0xff) as u8,
                ((u16::from(pix.g) * 256) >> 8) as u8,
                ((u16::from(pix.r) * 256) & 0xff) as u8,
                ((u16::from(pix.r) * 256) >> 8) as u8,
                ((u16::from(pix.b) * 256) & 0xff) as u8,
                ((u16::from(pix.b) * 256) >> 8) as u8,
            ])?;
        }
        writer.write_all(&[0xff, 0xff, 0xff, 0xf0])
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("hexws2811")
}

pub fn from_command(_: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    Ok(FromCommand::Device(Box::new(HexWS2811 {})))
}
