use std::io;
use clap;
use color::*;
use device::*;


pub struct Lpd8806 {}

impl Device for Lpd8806 {
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
        // FIXME: The number of zero bytes in the header and trailer should not be magic.
        writer.write_all(&[0x00; 10])?;
        for pix in pixels.iter().rev() {
            writer.write_all(&[(pix.g >> 1) | 0x80, (pix.r >> 1) | 0x80, (pix.b >> 1) | 0x80])?;
        }
        writer.write_all(&[0x00; 50])
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("lpd8806")
}

pub fn from_command(_: &clap::ArgMatches) -> Box<Device> {
    Box::new(Lpd8806 {})
}
