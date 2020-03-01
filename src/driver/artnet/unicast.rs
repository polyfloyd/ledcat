use super::target::*;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use nix::sys::socket;
use std::io;
use std::net;
use std::net::ToSocketAddrs;
use std::os::unix::io::FromRawFd;
use std::str;
use std::sync;
use std::thread;
use std::time;

pub const PORT: u16 = 6454;

pub struct Unicast {
    socket: net::UdpSocket,
    target: Box<dyn Target>,
    frame_size: usize,
    frame_buffer: Vec<u8>,
    universe: u16,
}

impl Unicast {
    pub fn to(target: Box<dyn Target>, frame_size: usize, universe: u16) -> io::Result<Unicast> {
        let socket = reuse_bind(("0.0.0.0", PORT))?;
        socket.set_broadcast(true)?;
        Ok(Unicast {
            socket,
            target,
            frame_size,
            frame_buffer: Vec::with_capacity(frame_size),
            universe,
        })
    }
}

impl io::Write for Unicast {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.frame_buffer.write(buf)?;
        self.flush()?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.frame_buffer.len() < self.frame_size {
            return Ok(());
        }
        let new_buf = self.frame_buffer.split_off(self.frame_size);
        let mut packet = Vec::new();
        art_dmx_packet(&mut packet, &self.frame_buffer, self.universe)?;
        self.frame_buffer = new_buf;
        for addr in self.target.addresses().iter() {
            self.socket.send_to(&packet, addr)?;
        }
        Ok(())
    }
}

pub fn discover() -> sync::mpsc::Receiver<io::Result<(net::SocketAddr, Option<String>)>> {
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

        let socket = try_or_send!(reuse_bind(("0.0.0.0", PORT)));
        try_or_send!(socket.set_broadcast(true));
        try_or_send!(socket.set_read_timeout(Some(time::Duration::from_secs(1))));

        loop {
            // Send out an ArtPoll packet to elicit an ArtPollReply from all devices in the network.
            let mut buf = Vec::new();
            try_or_send!(art_poll_packet(&mut buf));
            try_or_send!(socket.send_to(&buf, broadcast_addr()));

            loop {
                let mut recv_buf = [0; 231];
                let (_, sender_addr) = match socket.recv_from(&mut recv_buf) {
                    Err(_) => break,
                    Ok(rs) => rs,
                };
                if &recv_buf[0..8] != b"Art-Net\0" {
                    continue;
                }
                let mut rdr = io::Cursor::new(&recv_buf[8..10]);
                let opcode = try_or_send!(rdr.read_u16::<LittleEndian>());
                if opcode == 0x2100 {
                    let short_name = str::from_utf8(&recv_buf[19..38]).map(String::from).ok();
                    tx.send(Ok((sender_addr, short_name))).unwrap();
                }
            }
        }
    });

    rx
}

pub fn broadcast_addr() -> net::SocketAddr {
    ("255.255.255.255", PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap()
}

fn art_poll_packet<W>(mut wr: W) -> io::Result<()>
where
    W: io::Write,
{
    wr.write_all(b"Art-Net\0")?; // Artnet Header
    wr.write_u16::<LittleEndian>(0x2000)?; // OpCode
    wr.write_u8(4)?; // ProtVerHi
    wr.write_u8(14)?; // ProtVerLo
    wr.write_u8(0)?; // TalkToMe
    wr.write_u8(0x80)?; // Priority
    Ok(())
}

fn art_dmx_packet<W>(mut wr: W, data: &[u8], universe: u16) -> io::Result<()>
where
    W: io::Write,
{
    if data.len() >= 0xffff {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "data exceeds max dmx packet length",
        ));
    }
    wr.write_all(b"Art-Net\0")?; // Artnet Header
    wr.write_u16::<LittleEndian>(0x5000)?; // OpCode
    wr.write_u8(4)?; // ProtVerHi
    wr.write_u8(14)?; // ProtVerLo
    wr.write_u8(0)?; // Sequence
    wr.write_u8(0)?; // Physical
    wr.write_u8((universe & 0xff) as u8)?; // SubUni
    wr.write_u8((universe >> 8) as u8)?; // Net
    wr.write_u16::<BigEndian>(data.len() as u16)?; // Length
    wr.write_all(data)?; // Data
    Ok(())
}

/// Like `UdpSocket::bind`, but sets the socket reuse flags before binding.
#[cfg_attr(feature = "clippy", allow(needless_pass_by_value))]
fn reuse_bind<A: net::ToSocketAddrs>(to_addr: A) -> io::Result<net::UdpSocket> {
    let addr = to_addr.to_socket_addrs()?.next().unwrap();
    let fd = io_err!(socket::socket(
        socket::AddressFamily::Inet,
        socket::SockType::Datagram,
        socket::SockFlag::empty(),
        socket::SockProtocol::Udp,
    ))?;

    io_err!(socket::setsockopt(fd, socket::sockopt::ReuseAddr, &true))?;
    io_err!(socket::setsockopt(fd, socket::sockopt::ReusePort, &true))?;

    if addr.is_ipv6() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Artnet does not support IPv6 :(",
        ));
    }
    let sock_addr = socket::SockAddr::new_inet(socket::InetAddr::from_std(&addr));
    io_err!(socket::bind(fd, &sock_addr))?;

    Ok(unsafe { net::UdpSocket::from_raw_fd(fd) })
}
