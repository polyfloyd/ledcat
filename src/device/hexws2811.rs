use std::io;
use device::*;
use clap;


pub struct HexWS2811 {}

impl Device for HexWS2811 {
    fn clock_phase(&self) -> u8 {
        0
    }

    fn clock_polarity(&self) -> u8 {
        0
    }

    fn first_bit(&self) -> FirstBit {
        FirstBit::MSB
    }

    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        for pix in pixels.iter().rev() {
            writer.write_all(&[
                ((pix.g as u16 * 256) & 0xff) as u8,
                ((pix.g as u16 * 256) >> 8) as u8,
                ((pix.r as u16 * 256) & 0xff) as u8,
                ((pix.r as u16 * 256) >> 8) as u8,
                ((pix.b as u16 * 256) & 0xff) as u8,
                ((pix.b as u16 * 256) >> 8) as u8,
            ])?;
        }
        writer.write_all(&[0xff, 0xff, 0xff, 0xf0])
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("hexws2811")
}

pub fn from_command(_: &clap::ArgMatches) -> Box<Device> {
    Box::new(HexWS2811 {})
}
