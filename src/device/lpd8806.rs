use std::io;
use clap;
use color::*;
use device::*;


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
            first_bit: spidev::FirstBit::MSB,
            speed_hz: self.spidev_clock,
        })
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
        .arg(clap::Arg::with_name("spidev-clock")
            .long("spidev-clock")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .default_value("500000")
            .help("If spidev is used as driver, use this to set the clock frequency in Hertz"))
}

pub fn from_command(args: &clap::ArgMatches) -> Box<Device> {
    let spidev_clock = args.value_of("spidev-clock").unwrap()
        .parse().unwrap();
    Box::new(Lpd8806 {
        spidev_clock,
    })
}
