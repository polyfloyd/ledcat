use crate::device::*;
use std::collections;
use std::io::{self, Write};
use std::net;
use std::sync;
use std::thread;
use std::time;

mod target;
mod unicast;
use self::target::*;
use self::unicast::*;

pub fn command() -> clap::Command {
    clap::Command::new("artnet")
        .about("Control artnet DMX nodes via unicast and broadcast")
        .arg(clap::arg!(-t --target <value> ... "One or more target IP addresses")
            .value_parser(clap::value_parser!(net::IpAddr))
            .conflicts_with_all(["discover", "target-list", "broadcast"]))
        .arg(clap::arg!(--"target-list" <file> "Specify a file containing 1 IP address per line to unicast to. Changes to the file are read automatically")
            .conflicts_with_all(["target", "discover", "broadcast"]))
        .arg(clap::arg!(-b --broadcast "Broadcast to all devices in the network")
            .conflicts_with_all(["target", "target-list", "discover"]))
        .arg(clap::arg!(-d --discover "Discover artnet nodes")
            .conflicts_with_all(["target", "target-list", "broadcast"]))
        .arg(clap::arg!(-u --universe <value> "Discover artnet nodes")
            .value_parser(clap::value_parser!(u16))
            .default_value("0"))
}

pub fn from_command(args: &clap::ArgMatches, gargs: &GlobalArgs) -> io::Result<FromCommand> {
    if args.get_flag("discover") {
        if let Err(err) = artnet_discover() {
            eprintln!("{}", err);
        }
        return Ok(FromCommand::SubcommandHandled);
    }

    let dev = Box::new(generic::Generic {
        format: generic::Format::RGB24,
    });
    let artnet_target: Box<dyn Target> = if args.get_flag("broadcast") {
        Box::new(Broadcast {})
    } else if let Some(list_path) = args.get_one::<String>("target-list") {
        Box::new(ListFile::new(list_path))
    } else if let Some(targets) = args.get_many::<net::IpAddr>("target") {
        let addresses: Vec<_> = targets
            .map(|addr| net::SocketAddr::new(*addr, PORT))
            .collect();
        Box::new(addresses)
    } else {
        eprintln!("Missing artnet target. Please set --target IP or --broadcast");
        return Ok(FromCommand::SubcommandHandled);
    };
    let universe = args.get_one::<u16>("universe").unwrap();

    let output = Unicast::to(artnet_target, gargs.dimensions()?.size() * 3, *universe)?;
    Ok(FromCommand::Output(Box::new((dev, output))))
}

fn artnet_discover() -> io::Result<()> {
    let discovery_stream = unicast::discover();
    let mut discovered: collections::HashSet<net::SocketAddr> = collections::HashSet::new();

    let (close_tx, close_rx) = sync::mpsc::sync_channel(0);
    thread::spawn(move || {
        let mut out = io::stderr();
        for ch in ['|', '/', '-', '\\'].iter().cycle() {
            if close_rx.try_recv().is_ok() {
                break;
            }
            write!(&mut out, "\r{}", ch).unwrap();
            out.flush().unwrap();
            thread::sleep(time::Duration::from_millis(100));
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
