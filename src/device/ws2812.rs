use crate::color::*;
use crate::device::*;
use std::io;
use std::thread;
use std::time;

pub struct Ws2812 {}

impl Device for Ws2812 {
    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn spidev_config(&self) -> Option<spidev::Config> {
        Some(spidev::Config {
            clock_polarity: 0, // N/A: The WS2812 does not require a clock.
            clock_phase: 0,    // N/A
            first_bit: spidev::FirstBit::Msb,
            speed_hz: 2_400_000, // 1s / 1.25µs * 3 = 2.4MHz
        })
    }

    fn write_frame(&self, writer: &mut dyn io::Write, pixels: &[Pixel]) -> io::Result<()> {
        // 1.25 µs are required to transmit a single bit to the WS2812.
        // The value of a bit is determined by the duty cycle of a single period which
        // transitions from high to low. When this period is 1/3rd high, the bit is 0, when the
        // period is 2/3rds high, the bit is 1.
        // A single period is transmitted as 3 SPI bits of which the second bit determines the
        // duty cycle.
        let buf: Vec<u8> = pixels
            .iter()
            .flat_map(|pix| vec![pix.g, pix.r, pix.b])
            .flat_map(|b| {
                let mut obits: u32 = 0;
                for i in 0..8 {
                    let middle_bit = (b >> i) & 1;
                    obits |= (0b100 | u32::from(middle_bit << 1)) << (i * 3);
                }
                vec![
                    ((obits >> 16) & 0xff) as u8,
                    ((obits >> 8) & 0xff) as u8,
                    (obits & 0xff) as u8,
                ]
            })
            .collect();
        writer.write_all(&buf)?;
        thread::sleep(time::Duration::from_micros(50)); // Sleep for 50µs to reset.
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("ws2812")
}

pub fn from_command(_: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    Ok(FromCommand::Device(Box::new(Ws2812 {})))
}
