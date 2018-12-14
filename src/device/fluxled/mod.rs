mod bulb;

use self::bulb::*;
use clap;
use device::*;
use net2;
use net2::unix::UnixUdpBuilderExt;
use nix;
use std::collections;
use std::error;
use std::io::{self, Write};
use std::iter;
use std::net;
use std::str::FromStr;
use std::sync;
use std::thread;
use std::time;

const DISCOVERY_PORT: u16 = 48899;
const DISCOVERY_MAGIC: &[u8] = b"HF-A11ASSISTHREAD";

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("fluxled")
        .about("TODO")
        .arg(
            clap::Arg::with_name("target")
                .short("t")
                .long("target")
                .takes_value(true)
                .min_values(1)
                .multiple(true)
                .validator(|addr| match net::IpAddr::from_str(addr.as_str()) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(format!("{} ({})", err, addr)),
                })
                .conflicts_with_all(&["discover"])
                .help("One or more target IP addresses"),
        )
        .arg(
            clap::Arg::with_name("discover")
                .short("d")
                .long("discover")
                .conflicts_with_all(&["target"])
                .help("Discover Flux-LED nodes"),
        )
        .arg(
            clap::Arg::with_name("network")
                .short("n")
                .long("net")
                .takes_value(true)
                .requires_all(&["discover"])
                .help("The network range of where to look for devices in CIDR format"),
        )
}

pub fn from_command(args: &clap::ArgMatches, _gargs: &GlobalArgs) -> io::Result<FromCommand> {
    if args.is_present("discover") {
        let network_range_rs = args
            .value_of("network")
            .map(|cidr| {
                cidr.parse()
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            })
            .unwrap_or_else(Cidr::default_interface);
        let network_range = match network_range_rs {
            Ok(cidr) => cidr,
            Err(err) => {
                eprintln!(
                    "Could not guess which interface to use for discovery: {}",
                    err
                );
                eprintln!("Please set one using --net <cidr>");
                return Ok(FromCommand::SubcommandHandled);
            }
        };

        if let Err(err) = tui_discover(network_range) {
            eprintln!("{}", err);
        }
        return Ok(FromCommand::SubcommandHandled);
    }

    let bulbs: Vec<_> = args
        .values_of("target")
        .unwrap()
        .map(|addr| Bulb::new(addr.parse().unwrap()))
        .collect();

    let dev = Box::new(generic::Generic {
        format: generic::Format::RGB24,
    });
    let output = Display {
        bulbs,
        buf: Vec::new(),
    };
    Ok(FromCommand::Output(Box::new((dev, output))))
}

fn tui_discover(network_range: Cidr) -> io::Result<()> {
    let discovery_stream = discover(network_range);
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

fn discover(
    network_range: Cidr,
) -> sync::mpsc::Receiver<io::Result<(net::SocketAddr, Option<String>)>> {
    let (tx, rx) = sync::mpsc::channel();

    thread::spawn(move || {
        macro_rules! try_or_send {
            ($expression:expr) => {
                match $expression {
                    Ok(val) => val,
                    Err(err) => {
                        tx.send(Err(err)).unwrap();
                        return;
                    }
                }
            };
        }

        let socket = {
            let b = try_or_send!(net2::UdpBuilder::new_v4());
            try_or_send!(b.reuse_address(true));
            try_or_send!(b.reuse_port(true));
            try_or_send!(b.bind(("0.0.0.0", DISCOVERY_PORT)))
        };
        try_or_send!(socket.set_broadcast(true));
        try_or_send!(socket.set_read_timeout(Some(time::Duration::new(1, 0))));

        loop {
            for ip in network_range.addresses() {
                let addr = net::SocketAddr::new(net::IpAddr::V4(ip), DISCOVERY_PORT);
                try_or_send!(socket.send_to(DISCOVERY_MAGIC, addr));
            }

            loop {
                let mut recv_buf = [0; 64];
                let (_, sender_addr) = match socket.recv_from(&mut recv_buf) {
                    Err(_) => break,
                    Ok(rs) => rs,
                };
                if recv_buf.starts_with(DISCOVERY_MAGIC) {
                    // Filter out ourselves.
                    continue;
                }
                let name = String::from_utf8_lossy(&recv_buf).into_owned();
                tx.send(Ok((sender_addr, Some(name)))).unwrap();
            }
        }
    });

    rx
}

struct Cidr {
    addr: net::IpAddr,
    mask: net::IpAddr,
}

impl Cidr {
    fn addresses(&self) -> impl iter::Iterator<Item = net::Ipv4Addr> {
        match (self.addr, self.mask) {
            (net::IpAddr::V4(network_ip), net::IpAddr::V4(mask_ip)) => {
                let network: u32 = network_ip.into();
                let mask: u32 = mask_ip.into();
                let start = network & mask;
                let end = start | !mask;
                (start..end).map(net::Ipv4Addr::from)
            }
            (net::IpAddr::V6(_network), net::IpAddr::V6(_mask)) => unimplemented!(),
            _ => unreachable!(),
        }
    }

    #[cfg(target_os = "linux")]
    fn default_interface() -> io::Result<Cidr> {
        use nix::net::if_::InterfaceFlags;
        use nix::sys::socket::{InetAddr, SockAddr};
        nix::ifaddrs::getifaddrs()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
            // We need an interface with an address and mask configured.
            .filter(|iface| iface.address.is_some() && iface.netmask.is_some())
            // Filter out loopback interfaces, those are not very useful for discovering remote
            // devices.
            .filter(|iface| !iface.flags.contains(InterfaceFlags::IFF_LOOPBACK))
            // Find an interface which is actually connected to something.
            .filter(|iface| iface.flags.contains(InterfaceFlags::IFF_LOWER_UP))
            // Filter out IPv6-only interfaces, assume the devices we are trying to discover
            // are pieces of shit that only support IPv4.
            .filter(|iface| match iface.address {
                Some(SockAddr::Inet(InetAddr::V4(_))) => true,
                _ => false,
            })
            // Convert the interface's address to CIDR notation.
            .filter_map(|iface| {
                let addr = match iface.address.unwrap() {
                    SockAddr::Inet(a) => a.to_std().ip(),
                    _ => return None,
                };
                let mask = match iface.netmask.unwrap() {
                    SockAddr::Inet(a) => a.to_std().ip(),
                    _ => return None,
                };
                Some(Cidr { addr, mask })
            })
            // Pick the first interface that matches our criteria.
            .next()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "Unable to determine default network")
            })
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn default_interface() -> io::Result<Cidr> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Platform is not supported",
        ))
    }
}

impl FromStr for Cidr {
    type Err = Box<error::Error + Send + Sync>;
    fn from_str(s: &str) -> Result<Cidr, Self::Err> {
        let mut split = s.split('/');
        let addr: net::IpAddr = split
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "missing the address of the CIDR"))?
            .parse()?;
        let mask_str = split
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "missing the mask of the CIDR"))?;
        let mask: net::IpAddr =
            mask_str
                .parse()
                .or_else(|_| -> Result<_, Box<error::Error + Send + Sync>> {
                    let bits: u32 = mask_str.parse()?;
                    Ok(net::IpAddr::V4(net::Ipv4Addr::from(
                        !((0x8000_0000 >> (bits - 1)) - 1),
                    )))
                })?;
        Ok(Cidr { addr, mask })
    }
}
