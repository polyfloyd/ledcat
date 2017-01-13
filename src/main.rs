extern crate byteorder;
extern crate clap;
#[macro_use]
extern crate ioctl;
extern crate libc;
extern crate regex;

mod device;
mod driver;
mod input;

use std::borrow::Borrow;
use std::collections;
use std::io::Write;
use std::io;
use std::net;
use std::ops::Deref;
use std::path;
use std::str::FromStr;
use std::sync;
use std::thread;
use std::time;
use regex::Regex;
use device::*;
use driver::*;
use input::*;

macro_rules! regex_validator {
    ($expression:expr) => ({
        let ex = Regex::new($expression).unwrap();
        move |val: String| {
            if ex.is_match(val.as_str()) {
                Ok(())
            } else {
                Err(format!("\"{}\" does not match {}", val, ex))
            }
        }
    })
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
        .arg(clap::Arg::with_name("input")
            .short("i")
            .long("input")
            .takes_value(true)
            .min_values(1)
            .multiple(true)
            .default_value("-")
            .help("The inputs to read from. Read the manual for how inputs are read and prioritized."))
        .arg(clap::Arg::with_name("async")
            .long("async")
            .requires("framerate")
            .help("Instead of synchronously reading from one input at a time, consume all data concurrently, possibly dropping frames."))
        .arg(clap::Arg::with_name("num-pixels")
            .short("n")
            .long("num-pixels")
            .global(true)
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
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
            .validator(regex_validator!(r"^[1-9]\d*$"))
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
                 .min_values(1)
                 .multiple(true)
                 .validator(|addr| match net::IpAddr::from_str(addr.as_str()) {
                     Ok(_)    => Ok(()),
                     Err(err) => Err(format!("{} ({})", err, addr)),
                 })
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

    let matches = cli.clone().get_matches();
    let (sub_name, sub_matches) = matches.subcommand();
    if sub_name == "" {
        let mut out = io::stderr();
        cli.write_help(&mut out).unwrap();
        writeln!(out, "").unwrap();
        return;
    }

    if sub_name == "artnet" && sub_matches.unwrap().is_present("discover") {
        if let Err(err) = artnet_discover() {
            writeln!(io::stderr(), "{}", err).unwrap();
        }
        return;
    }

    let num_pixels = match matches.value_of("num-pixels") {
        Some(s) => s.parse::<usize>().unwrap(),
        None    => {
            writeln!(io::stderr(), "--num-pixels is unset").unwrap();
            return;
        }
    };

    let (mut output, dev) = if sub_name == "artnet" {
        let dev: Box<Device> = Box::new(device::raw::Raw{ clock_phase: 0, clock_polarity: 0, first_bit: FirstBit::MSB });
        let artnet_addrs = if sub_matches.unwrap().is_present("broadcast") {
            vec![ artnet::broadcast_addr() ]
        } else {
            sub_matches.unwrap().values_of("target").unwrap().map(|addr| {
                net::SocketAddr::new(net::IpAddr::from_str(addr).unwrap(), artnet::PORT)
            }).collect()
        };
        let output: Box<io::Write> = match artnet::Unicast::to(artnet_addrs, num_pixels * 3) {
            Ok(out)  => Box::new(out),
            Err(err) => {
                writeln!(io::stderr(), "{}", err).unwrap();
                return;
            },
        };
        (output, dev)

    } else {
        let dev = device_constructors[sub_name](sub_matches.unwrap());
        let output_file = path::PathBuf::from(match matches.value_of("output").unwrap() {
            "-" => "/dev/stdout",
            _   => matches.value_of("output").unwrap(),
        });

        let driver_name = matches.value_of("driver")
            .map(|s: &str| s.to_string())
            .or(driver::detect(&output_file));
        let driver_name = match driver_name {
            Some(n) => n,
            None    => {
                writeln!(io::stderr(), "Unable to determine the driver to use. Please set one using --driver.").unwrap();
                return;
            },
        };
        let output: Box<io::Write> = match driver_name.as_str() {
            "spidev" => Box::new(spidev::open(&output_file, dev.borrow(), 4_000_000).unwrap()),
            _        => {
                writeln!(io::stderr(), "Unknown driver {}", driver_name).unwrap();
                return;
            },
        };
        (output, dev)
    };

    let frame_interval = matches.value_of("framerate").map(|fps| {
        time::Duration::new(1, 0) / fps.parse::<u32>().unwrap()
    });
    let limit_framerate: Box<Fn()> = match frame_interval.clone() {
        Some(interval) => Box::new(move || std::thread::sleep(interval)),
        None           => Box::new(|| ()),
    };
    let single_frame = matches.is_present("single-frame");

    let inputs = matches.values_of("input").unwrap();
    let input_consume = if matches.is_present("async") {
         select::Consume::All(frame_interval.unwrap())
    } else {
         select::Consume::Single
    };
    let mut input = select::Reader::from_files(inputs.map(|f| {
        match f { "-" => "/dev/stdin", f => f }
    }).collect(), num_pixels * 3, input_consume).unwrap();

    if single_frame {
        let _ = pipe_frame(&mut input, &mut output, dev.deref(), num_pixels);
    } else {
        loop {
            if let Err(_) = pipe_frame(&mut input, &mut output, dev.deref(), num_pixels) {
                break;
            }
            limit_framerate();
        }
    }
}

fn pipe_frame(mut input: &mut io::Read, mut output: &mut io::Write, dev: &Device, num_pixels: usize) -> io::Result<()> {
    // Read a full frame into a buffer. This prevents half frames being written to a
    // potentially timing sensitive output if the input blocks.
    let mut buffer = Vec::with_capacity(num_pixels);
    for _ in 0..num_pixels {
        buffer.push(try!(Pixel::read_rgb24(&mut input)));
    }
    dev.write_frame(&mut output, &buffer)
}

fn artnet_discover() -> io::Result<()> {
    let discovery_stream = artnet::discover();
    let mut discovered: collections::HashSet<net::SocketAddr> = collections::HashSet::new();

    let (close_tx, close_rx) = sync::mpsc::sync_channel(0);
    thread::spawn(move || {
        let mut out = io::stderr();
        for ch in ['|', '/', '-', '\\'].iter().cycle() {
            if let Ok(_) = close_rx.try_recv() {
                break;
            }
            write!(&mut out, "\r{}", ch).unwrap();
            out.flush().unwrap();
            thread::sleep(time::Duration::new(0, 100_000_000));
        }
    });

    let mut out = io::stderr();
    for result in discovery_stream {
        let node = match result {
            Ok(node) => node,
            Err(err) => {
                close_tx.send(()).unwrap();
                write!(&mut out, "\r").unwrap();
                return Err(err);
            },
        };
        if !discovered.contains(&node.0) {
            let ip_str = format!("{}", node.0.ip()); // Padding only works with strings. :(
            try!(match node.1 {
                Some(name) => writeln!(out, "\r{: <15} -> {}", ip_str, name),
                None       => writeln!(out, "\r{: <15}", ip_str),
            });
        }
        discovered.insert(node.0);
    }
    Ok(())
}
