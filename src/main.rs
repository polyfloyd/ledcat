extern crate clap;
#[macro_use]
extern crate ioctl;
extern crate regex;

mod device;
mod driver;

use std::collections;
use std::io;
use std::net;
use std::path;
use std::str::FromStr;
use std::time;
use device::*;
use driver::*;

fn is_int(s: String) -> Result<(), String> {
    match s.parse::<u64>() {
        Ok(_)  => Ok(()),
        Err(_) => Err("Value is not a positive integer".to_string()),
    }
}

fn framerate_limiter(opt: Option<&str>) -> Box<Fn()> {
    match opt {
        Some(fps) => {
            let dur = time::Duration::new(1, 0) / fps.parse::<u32>().unwrap();
            return Box::new(move || std::thread::sleep(dur))
        },
        None => return Box::new(|| ()),
    };
}

fn main() {
    let mut cli = clap::App::new("ledcat")
        .version("0.0.1")
        .author("polyfloyd <floyd@polyfloyd.net>")
        .about("Like netcat, but for leds.")
        .arg(clap::Arg::with_name("output")
            .short("o")
            .long("output")
            .takes_value(true)
            .default_value("-")
            .help("The output file to write to. Use - for stdout."))
        .arg(clap::Arg::with_name("pixels")
            .short("n")
            .long("pixels")
            .required(true)
            .takes_value(true)
            .validator(is_int)
            .value_name("num pixels")
            .help("The number of pixels in the string"))
        .arg(clap::Arg::with_name("driver")
            .long("driver")
            .help("The driver to use for the output. If this is not specified, the driver is automaticaly detected based on the output"))
        .arg(clap::Arg::with_name("framerate")
            .short("f")
            .long("framerate")
            .takes_value(true)
            .validator(is_int)
            .help("Limit the number of frames per second"))
        .arg(clap::Arg::with_name("single-frame")
            .short("1")
            .long("one")
            .conflicts_with("framerate")
            .help("Send a single frame to the output and exit")));

    let mut device_constructors = collections::HashMap::new();
    for device_init in device::devices() {
        device_constructors.insert(device_init.0.get_name().to_string(), device_init.1);
        cli = cli.subcommand(device_init.0);
    }
    let matches = cli.get_matches();
    let (sub_name, sub_matches) = matches.subcommand();

    let dev = device_constructors[sub_name](sub_matches.unwrap());
    let output_file = match matches.value_of("output").unwrap() {
        "-" => "/dev/stdout",
        _   => matches.value_of("output").unwrap(),
    };
    let output_file = path::PathBuf::from(output_file);

    let driver_name = match matches.value_of("driver") {
        Some(driver) => Some(driver.to_string()),
        None         => driver::detect(&output_file),
    };
    if driver_name.is_none() {
        println!("Unable to determine the driver to use. Please set one using --driver.");
        return;
    }
    let driver_name = driver_name.unwrap();

    let num_pixels = matches.value_of("pixels").unwrap().parse::<usize>().unwrap();
    let limit_framerate = framerate_limiter(matches.value_of("framerate"));
    let single_frame = matches.is_present("single-frame");

    let mut out = spidev::open(output_file, dev.borrow(), 4_000_000).unwrap();

    let mut input = io::stdin();
    loop {
        // Read a full frame into a buffer. This prevents half frames being written to a
        // potentially timing sensitive output if the input blocks.
        let mut buffer = Vec::with_capacity(num_pixels);
        for _ in 0..num_pixels {
            buffer.push(Pixel::read_rgb24(&mut input).unwrap());
        }
        dev.write_frame(&mut out, &buffer).unwrap();
        limit_framerate();

        if single_frame {
            break;
        }
    }
}
