use std::io;
use device::*;
use clap;

pub struct Raw {
    clock_phase:    u8,
    clock_polarity: u8,
    first_bit:      FirstBit,
}

impl Device for Raw {

    fn clock_phase(&self) -> u8 {
        self.clock_phase
    }

    fn clock_polarity(&self) -> u8 {
        self.clock_polarity
    }

    fn first_bit(&self) -> FirstBit {
        self.first_bit.clone()
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        for pix in pixels.iter() {
            try!(writer.write_all(&[pix.g, pix.r, pix.b]));
        }
        Ok(())
    }

}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("raw")
        .about("Output data as RGB24")
        .arg(clap::Arg::with_name("clock-phase")
             .short("a")
             .long("cpha")
             .takes_value(true)
             .possible_values(&["0", "1"])
             .help("Clock phase"))
        .arg(clap::Arg::with_name("clock-polarity")
             .short("o")
             .long("cpol")
             .takes_value(true)
             .possible_values(&["0", "1"])
             .help("Clock polarity"))
        .arg(clap::Arg::with_name("first-bit")
             .short("b")
             .long("firstbit")
             .takes_value(true)
             .possible_values(&["msb", "lsb"])
             .help("First bit"))
}

pub fn from_command(args: &clap::ArgMatches) -> Box<Device> {
    let cpha = args.value_of("clock-phase").unwrap_or("0").parse::<u8>().unwrap();
    let cpol = args.value_of("clock-polarity").unwrap_or("0").parse::<u8>().unwrap();
    let fb = match args.value_of("first-bit").unwrap_or("msb") {
        "msb" => FirstBit::MSB,
        _     => FirstBit::LSB,
    };
    Box::new(Raw {
        clock_phase: cpha,
        clock_polarity: cpol,
        first_bit: fb,
    })
}
