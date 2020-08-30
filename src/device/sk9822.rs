use crate::color::*;
use crate::device::*;
use std::io;

pub struct Sk9822 {
    /// 5-bit grayscale value to apply to all pixels.
    pub grayscale: u8,
    pub spidev_clock: u32,
}

impl Device for Sk9822 {
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

    fn write_frame(&self, writer: &mut dyn io::Write, pixels: &[Pixel]) -> io::Result<()> {
        writer.write_all(&[0x00; 4])?;
        for pix in pixels {
            writer.write_all(&[0b1110_0000 | self.grayscale, pix.b, pix.g, pix.r])?;
        }
        writer.write_all(&[0xff; 4])?;
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("sk9822")
        .arg(
            clap::Arg::with_name("grayscale")
                .short("g")
                .long("grayscale")
                .validator(validate_grayscale)
                .default_value("31")
                .help("Set the 5-bit grayscale for all pixels"),
        )
        .arg(
            clap::Arg::with_name("spidev-clock")
                .long("spidev-clock")
                .takes_value(true)
                .validator(regex_validator!(r"^[1-9]\d*$"))
                .default_value("500000")
                .help("If spidev is used as driver, use this to set the clock frequency in Hertz"),
        )
}

pub fn from_command(args: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    let grayscale = args.value_of("grayscale").unwrap().parse().unwrap();
    let spidev_clock = args.value_of("spidev-clock").unwrap().parse().unwrap();
    Ok(FromCommand::Device(Box::new(Sk9822 {
        grayscale,
        spidev_clock,
    })))
}

fn validate_grayscale(v: String) -> Result<(), String> {
    match v.parse::<u8>() {
        Ok(i) => {
            if i <= 31 {
                Ok(())
            } else {
                Err(format!("Grayscale value out of range: 0 <= {} <= 31", i))
            }
        }
        Err(e) => Err(format!("{}", e)),
    }
}
