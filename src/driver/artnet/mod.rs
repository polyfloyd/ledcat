use std::collections;
use std::io::{self, Write};
use std::net;
use std::str::{self, FromStr};
use std::sync;
use std::thread;
use std::time;
use clap;
use ::device;
use ::geometry;

mod unicast;
mod target;
use self::unicast::*;
use self::target::*;


pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("artnet")
        .about("Control artnet DMX nodes via unicast and broadcast")
        .arg(clap::Arg::with_name("target")
            .short("t")
            .long("target")
            .takes_value(true)
            .min_values(1)
            .multiple(true)
            .validator(|addr| match net::IpAddr::from_str(addr.as_str()) {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("{} ({})", err, addr)),
            })
            .conflicts_with_all(&["discover", "target-list", "broadcast"])
            .help("One or more target IP addresses"))
        .arg(clap::Arg::with_name("target-list")
            .long("target-list")
            .takes_value(true)
            .conflicts_with_all(&["target", "discover", "broadcast"])
            .help("Specify a file containing 1 IP address per line to unicast to. \
                   Changes to the file are read automatically"))
        .arg(clap::Arg::with_name("broadcast")
            .short("b")
            .long("broadcast")
            .conflicts_with_all(&["target", "target-list", "discover"])
            .help("Broadcast to all devices in the network"))
        .arg(clap::Arg::with_name("discover")
            .short("d")
            .long("discover")
            .conflicts_with_all(&["target", "target-list", "broadcast"])
            .help("Discover artnet nodes"))
}

pub fn from_command(args: &clap::ArgMatches, maybe_dimensions: Option<geometry::Dimensions>) -> io::Result<Option<Box<device::Output>>> {
    if args.is_present("discover") {
        if let Err(err) = artnet_discover() {
            eprintln!("{}", err);
        }
        return Ok(None);
    }

    let dev = Box::new(device::generic::Generic {});
    let artnet_target: Box<Target> = if args.is_present("broadcast") {
        Box::new(Broadcast{})
    } else if let Some(list_path) = args.value_of("target-list") {
        Box::new(ListFile::new(list_path))
    } else {
        let addresses: Vec<_> = args.values_of("target").unwrap().map(|addr| {
            net::SocketAddr::new(addr.parse().unwrap(), PORT)
        }).collect();
        Box::new(addresses)
    };

    let dimensions = match maybe_dimensions {
        Some(d) => d,
        None => {
            eprintln!("Please set the frame size through either --num-pixels or --geometry");
            return Ok(None);
        },
    };

    let output = Unicast::to(artnet_target, dimensions.size() * 3)?;
    Ok(Some(Box::new((dev, output))))
}

fn artnet_discover() -> io::Result<()> {
    let discovery_stream = unicast::discover();
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

    for result in discovery_stream {
        let node = match result {
            Ok(node) => node,
            Err(err) => {
                close_tx.send(()).unwrap();
                eprint!("\r");
                return Err(err);
            }
        };
        if !discovered.contains(&node.0) {
            let ip_str = format!("{}", node.0.ip()); // Padding only works with strings. :(
            match node.1 {
                Some(name) => eprintln!("\r{: <15} -> {}", ip_str, name),
                None => eprintln!("\r{: <15}", ip_str),
            };
        }
        discovered.insert(node.0);
    }
    Ok(())
}
