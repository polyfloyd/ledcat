use crate::color::*;
use crate::device::*;
use std::io;

pub struct Lpd8806 {
    pub spidev_clock: u32,
}

impl Device for Lpd8806 {
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
        // FIXME: The number of zero bytes in the header and trailer should not be magic.
        writer.write_all(&[0x00; 10])?;
        for pix in pixels.iter().rev() {
            writer.write_all(&[
                (pix.g >> 1) | 0x80,
                (pix.r >> 1) | 0x80,
                (pix.b >> 1) | 0x80,
            ])?;
        }
        writer.write_all(&[0x00; 50])
    }
}

pub fn command() -> clap::Command {
    clap::Command::new("lpd8806").arg(
        clap::arg!(--"spidev-clock" <value> "If spidev is used as driver, use this to set the clock frequency in Hertz")
            .value_parser(clap::value_parser!(u32))
            .default_value("500000"),
    )
}

pub fn from_command(args: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    let spidev_clock = *args.get_one::<u32>("spidev-clock").unwrap();
    Ok(FromCommand::Device(Box::new(Lpd8806 { spidev_clock })))
}
