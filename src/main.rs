extern crate clap;
#[macro_use]
extern crate ioctl;

mod device;
mod driver;

use std::io;
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

fn detect_driver<'f>(file: &'f str) -> &'static str {
    "spidev"
}

fn main() {
    let matches = clap::App::new("ledcat")
        .version("0.0.1")
        .author("polyfloyd <floyd@polyfloyd.net>")
        .about("Like netcat, but for leds.")
        .arg(clap::Arg::with_name("type")
            .short("t")
            .long("type")
            .required(true)
            .takes_value(true)
            .value_name("device type")
            .help("The led-device type"))
        .arg(clap::Arg::with_name("pixels")
            .short("n")
            .long("pixels")
            .required(true)
            .takes_value(true)
            .validator(is_int)
            .help("The number of pixels in the string"))
        .arg(clap::Arg::with_name("driver")
            .long("driver")
            .help("The driver to use for the output. If this is not specified, the driver is automaticaly detected."))
        .arg(clap::Arg::with_name("output")
            .short("o")
            .long("output")
            .takes_value(true)
            .default_value("-")
            .help("The output file to write to. Use - for stdout."))
        .arg(clap::Arg::with_name("framerate")
            .short("f")
            .long("framerate")
            .takes_value(true)
            .validator(is_int)
            .help("Limit the number of frames per second"))
        .get_matches();

    let device_type = matches.value_of("type").unwrap();
    let output_file = match matches.value_of("output").unwrap() {
        "-" => "/dev/stdout",
        _   => matches.value_of("output").unwrap(),
    };
    let driver_name = match matches.value_of("driver") {
        Some(driver) => driver,
        None         => detect_driver(output_file),
    };
    let num_pixels = matches.value_of("pixels").unwrap().parse::<usize>().unwrap();
    let limit_framerate = framerate_limiter(matches.value_of("framerate"));

    let dev = match device_type {
        "apa102" => device::apa102::Apa102{},
        _ => {
            println!("Unknown device type: {}", device_type);
            return;
        },
    };
    let mut out = spidev::open(output_file, &dev).unwrap();

    loop {
        // Read a full frame into a buffer. This prevents half frames being written to a
        // potentially timing sensitive output if the input blocks.
        let mut buffer = Vec::with_capacity(num_pixels);
        let mut input = io::stdin();
        for _ in 0..num_pixels {
            buffer.push(Pixel::read_rgb24(&mut input).unwrap());
        }

        dev.begin_frame(&mut out).unwrap();
        for i in 0..num_pixels {
            dev.write_pixel(&mut out, &buffer[i]).unwrap();
        }
        dev.end_frame(&mut out).unwrap();

        limit_framerate();
    }
}
