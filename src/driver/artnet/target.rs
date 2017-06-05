use std::borrow::Cow;
use std::net;


pub trait Target {
    fn addresses<'a>(&'a self) -> Cow<'a, [net::SocketAddr]>;
}


impl Target for Vec<net::SocketAddr> {
    fn addresses<'a>(&'a self) -> Cow<'a, [net::SocketAddr]> {
        Cow::Borrowed(&self)
    }
}


pub struct Broadcast {}

impl Target for Broadcast {
    fn addresses<'a>(&'a self) -> Cow<'a, [net::SocketAddr]> {
        let ip = net::Ipv4Addr::new(255, 255, 255, 255);
        let addrs = vec![net::SocketAddrV4::new(ip, super::PORT).into()];
        Cow::Owned(addrs)
    }
}
