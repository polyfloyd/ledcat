extern crate byteorder;
extern crate clap;
#[macro_use]
extern crate ioctl;
extern crate libc;
extern crate regex;

mod device;
mod driver;

use std::borrow::Borrow;
use std::collections;
use std::io;
use std::net;
use std::ops::Deref;
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
            .takes_value(true)
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
            .help("Send a single frame to the output and exit"))
        .subcommand(clap::SubCommand::with_name("artnet")
            .about("Control LEDs over artnet unicast and broadcast")
            .arg(clap::Arg::with_name("target")
                 .short("t")
                 .long("target")
                 .takes_value(true)
                 .conflicts_with_all(&["discover", "broadcast"])
                 .help("The target IP address"))
            .arg(clap::Arg::with_name("discover")
                 .short("d")
                 .long("discover")
                 .conflicts_with_all(&["target", "broadcast"])
                 .help("Discover artnet nodes"))
            .arg(clap::Arg::with_name("broadcast")
                 .short("b")
                 .long("broadcast")
                 .conflicts_with_all(&["target", "discover"])
                 .help("Broadcast to all devices in the network"))
            .help("Control artnet DMX nodes via unicast and broadcast"));

    let mut device_constructors = collections::HashMap::new();
    for device_init in device::devices() {
        device_constructors.insert(device_init.0.get_name().to_string(), device_init.1);
        cli = cli.subcommand(device_init.0);
    }
    let matches = cli.get_matches();
    let (sub_name, sub_matches) = matches.subcommand();

    if sub_name == "artnet" {
        if sub_matches.unwrap().is_present("discover") {
            let discovered = artnet::discover(time::Duration::new(3, 0)).unwrap();
            for addr in discovered {
                println!("  {:?}", addr);
            }
            return;
        }
    }

    let (mut output, dev) = if sub_name == "artnet" {
        let dev: Box<Device> = Box::new(device::raw::Raw{ clock_phase: 0, clock_polarity: 0, first_bit: FirstBit::MSB });
        let artnet_addr = if sub_matches.unwrap().is_present("broadcast") {
            artnet::broadcast_addr()
        } else {
            match net::IpAddr::from_str(sub_matches.unwrap().value_of("target").unwrap()) {
                Ok(a)  => net::SocketAddr::new(a, artnet::PORT),
                Err(_) => {
                    println!("Invalid IP address");
                    return;
                },
            }
        };
        let num_pixels = matches.value_of("pixels").unwrap().parse::<usize>().unwrap();
        let output: Box<io::Write> = Box::new(artnet::Unicast::to(artnet_addr, num_pixels * 3).unwrap());
        (output, dev)

    } else {
        let dev = device_constructors[sub_name](sub_matches.unwrap());
        let output_file = path::PathBuf::from(match matches.value_of("output").unwrap() {
            "-" => "/dev/stdout",
            _   => matches.value_of("output").unwrap(),
        });
        let driver_name = match matches.value_of("driver") {
            Some(driver) => Some(driver.to_string()),
            None         => driver::detect(&output_file),
        };
        let driver_name = match driver_name {
            Some(n) => n,
            None    => {
                println!("Unable to determine the driver to use. Please set one using --driver.");
                return;
            },
        };
        let output: Box<io::Write> = match driver_name.as_str() {
            "spidev" => Box::new(spidev::open(&output_file, dev.borrow(), 4_000_000).unwrap()),
            _        => {
                println!("Unsupported or unknown driver: {}", driver_name);
                return;
            },
        };
        (output, dev)
    };

    let num_pixels = matches.value_of("pixels").unwrap().parse::<usize>().unwrap();
    let limit_framerate: Box<Fn()> = match matches.value_of("framerate") {
        Some(fps) => {
            let dur = time::Duration::new(1, 0) / fps.parse::<u32>().unwrap();
            Box::new(move || std::thread::sleep(dur))
        },
        None => Box::new(|| ()),
    };
    let single_frame = matches.is_present("single-frame");

    let mut input = io::stdin();
    if single_frame {
        pipe_frame(&mut input, &mut output, dev.deref(), num_pixels);
    } else {
        loop {
            pipe_frame(&mut input, &mut output, dev.deref(), num_pixels);
            limit_framerate();
        }
    }
}

fn pipe_frame(mut input: &mut io::Read, mut output: &mut io::Write, dev: &Device, num_pixels: usize) {
    // Read a full frame into a buffer. This prevents half frames being written to a
    // potentially timing sensitive output if the input blocks.
    let mut buffer = Vec::with_capacity(num_pixels);
    for _ in 0..num_pixels {
        buffer.push(Pixel::read_rgb24(&mut input).unwrap());
    }
    dev.write_frame(&mut output, &buffer).unwrap();
}
