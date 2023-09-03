use crate::color::*;
use crate::device::*;
use std::io;

pub struct Apa102 {
    /// 5-bit grayscale value to apply to all pixels.
    pub grayscale: u8,
    pub spidev_clock: u32,
}

impl Device for Apa102 {
    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn spidev_config(&self) -> Option<spidev::Config> {
        Some(spidev::Config {
            clock_polarity: 0,
            clock_phase: 0,
            first_bit: spidev::FirstBit::Msb,
            speed_hz: self.spidev_clock,
        })
    }

    fn write_frame(&self, writer: &mut dyn io::Write, pixels: &[Pixel]) -> io::Result<()> {
        writer.write_all(&[0x00; 4])?;
        for pix in pixels {
            writer.write_all(&[0b1110_0000 | self.grayscale, pix.b, pix.g, pix.r])?;
        }
        Ok(())
    }
}

pub fn command() -> clap::Command {
    clap::Command::new("apa102")
        .arg(clap::arg!(-g --grayscale "Set the 5-bit grayscale for all pixels")
                .value_parser(clap::value_parser!(u8).range(0..32))
                .default_value("31"))
        .arg(clap::arg!(--"spidev-clock" <value> "If spidev is used as driver, use this to set the clock frequency in Hertz")
                .value_parser(clap::value_parser!(u32))
                .default_value("500000"))
}

pub fn from_command(args: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    let grayscale = *args.get_one::<u8>("grayscale").unwrap();
    let spidev_clock = *args.get_one::<u32>("spidev-clock").unwrap();
    Ok(FromCommand::Device(Box::new(Apa102 {
        grayscale,
        spidev_clock,
    })))
}
