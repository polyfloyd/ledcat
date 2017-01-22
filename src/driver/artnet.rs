use std::io;
use std::mem;
use std::net::ToSocketAddrs;
use std::net;
use std::os::unix::io::FromRawFd;
use std::str;
use std::sync;
use std::thread;
use std::time;
use libc;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};

pub const PORT: u16 = 6454;

pub struct Unicast {
    socket:       net::UdpSocket,
    to_addr:      Vec<net::SocketAddr>,
    frame_size:   usize,
    frame_buffer: Vec<u8>,
}

impl Unicast {

    pub fn to(addr: Vec<net::SocketAddr>, frame_size: usize) -> io::Result<Unicast> {
        let socket = try!(unsafe { reuse_bind(("0.0.0.0", PORT)) });
        try!(socket.set_broadcast(true));
        Ok(Unicast {
            socket:       socket,
            to_addr:      addr,
            frame_size:   frame_size,
            frame_buffer: Vec::with_capacity(frame_size),
        })
    }

}

impl io::Write for Unicast {

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = try!(self.frame_buffer.write(buf));
        try!(self.flush());
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.frame_buffer.len() < self.frame_size {
            return Ok(());
        }
        let new_buf = self.frame_buffer.split_off(self.frame_size);
        let mut packet = Vec::new();
        try!(art_dmx_packet(&mut packet, &self.frame_buffer));
        self.frame_buffer = new_buf;
        for addr in &self.to_addr {
            try!(self.socket.send_to(&packet, addr));
        }
        Ok(())
    }

}

pub fn broadcast_addr() -> net::SocketAddr {
    ("255.255.255.255", PORT).to_socket_addrs().unwrap().next().unwrap()
}

pub fn discover() -> sync::mpsc::Receiver<io::Result<(net::SocketAddr, Option<String>)>> {
    let (tx, rx) = sync::mpsc::channel();

    thread::spawn(move || {
        macro_rules! try_or_send {
            ($expression:expr) => (
                match $expression {
                    Ok(val)  => val,
                    Err(err) => {
                        tx.send(Err(err)).unwrap();
                        return;
                    }
                }
            )
        }

        let socket = try_or_send!(unsafe { reuse_bind(("0.0.0.0", PORT)) });
        try_or_send!(socket.set_broadcast(true));
        try_or_send!(socket.set_read_timeout(Some(time::Duration::new(1, 0))));

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
                    let short_name = str::from_utf8(&recv_buf[19..38])
                        .map(String::from)
                        .ok();
                    tx.send(Ok((sender_addr, short_name))).unwrap();
                }
            }
        }
    });

    rx
}

fn art_poll_packet(wr: &mut io::Write) -> io::Result<()> {
    try!(wr.write(b"Art-Net\0"));               // Artnet Header
    try!(wr.write_u16::<LittleEndian>(0x2000)); // OpCode
    try!(wr.write_u8(4));                       // ProtVerHi
    try!(wr.write_u8(14));                      // ProtVerLo
    try!(wr.write_u8(0));                       // TalkToMe
    try!(wr.write_u8(0x80));                    // Priority
    Ok(())
}

fn art_dmx_packet(wr: &mut io::Write, data: &Vec<u8>) -> io::Result<()> {
    if data.len() > 0xffff {
        return Err(io::Error::new(io::ErrorKind::Other, "data exceeds max dmx packet length"));
    }
    try!(wr.write(b"Art-Net\0"));                       // Artnet Header
    try!(wr.write_u16::<LittleEndian>(0x5000));         // OpCode
    try!(wr.write_u8(4));                               // ProtVerHi
    try!(wr.write_u8(14));                              // ProtVerLo
    try!(wr.write_u8(0));                               // Sequence
    try!(wr.write_u8(0));                               // Physical
    try!(wr.write_u8(0));                               // SubUni
    try!(wr.write_u8(0));                               // Net
    try!(wr.write_u16::<BigEndian>(data.len() as u16)); // Length
    for b in data {
        try!(wr.write_u8(*b));                          // Data
    }
    Ok(())
}

/// Like UdpSocket::bind, but sets the socket reuse flags before binding.
unsafe fn reuse_bind<A: net::ToSocketAddrs>(to_addr: A) -> io::Result<net::UdpSocket> {
    let addr = try!(to_addr.to_socket_addrs()).next().unwrap(); // TODO: Use the other addresses.

    let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let yes: u32 = 1;
    let yes_ptr = &yes as *const _ as *const libc::c_void;
    if libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, yes_ptr, 4) == -1 {
        let err = io::Error::last_os_error();
        libc::close(fd);
        return Err(err);
    }
    if libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEPORT, yes_ptr, 4) == -1 {
        let err = io::Error::last_os_error();
        libc::close(fd);
        return Err(err);
    }

    let sock_addr: libc::sockaddr_in = match addr {
        net::SocketAddr::V4(addr) => libc::sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port:   (addr.port()>>8) | (addr.port()<<8), // WTF
            sin_addr:   libc::in_addr {
                s_addr: io::Cursor::new(addr.ip().octets()).read_u32::<BigEndian>().unwrap(),
            },
            sin_zero:   [0; 8],
        },
        net::SocketAddr::V6(_) => unimplemented!(), // TODO
    };
    let rt = libc::bind(fd, &sock_addr as *const _ as *const libc::sockaddr, mem::size_of::<libc::sockaddr_in>() as u32);
    if rt == -1 {
        libc::close(fd);
        return Err(io::Error::last_os_error());
    }
    return Ok(net::UdpSocket::from_raw_fd(fd));
}
