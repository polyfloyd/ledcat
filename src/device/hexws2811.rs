use crate::device::*;
use std::io::{self, Write};

pub struct HexWS2811 {}

impl Device for HexWS2811 {
    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn write_frame(&self, writer: &mut dyn io::Write, pixels: &[Pixel]) -> io::Result<()> {
        let mut buf = Vec::with_capacity(pixels.len() * 6 + 4);
        for pix in pixels.iter().rev() {
            buf.write_all(&[
                ((u16::from(pix.g) * 256) & 0xff) as u8,
                ((u16::from(pix.g) * 256) >> 8) as u8,
                ((u16::from(pix.r) * 256) & 0xff) as u8,
                ((u16::from(pix.r) * 256) >> 8) as u8,
                ((u16::from(pix.b) * 256) & 0xff) as u8,
                ((u16::from(pix.b) * 256) >> 8) as u8,
            ])?;
        }
        buf.write_all(&[0xff, 0xff, 0xff, 0xf0])?;
        writer.write_all(&buf)
    }
}

pub fn command() -> clap::Command {
    clap::Command::new("hexws2811")
}

pub fn from_command(_: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    Ok(FromCommand::Device(Box::new(HexWS2811 {})))
}
