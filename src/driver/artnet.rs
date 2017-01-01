use std::io;
use std::net;
use std::time;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};

pub const PORT: u16 = 6454;

pub struct Unicast {
    socket:       net::UdpSocket,
    to_addr:      net::SocketAddr,
    frame_size:   usize,
    frame_buffer: Vec<u8>,
}

impl Unicast {

    pub fn to(addr: net::SocketAddr, frame_size: usize) -> io::Result<Unicast> {
        let socket = try!(net::UdpSocket::bind(("0.0.0.0", PORT)));
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
        try!(self.socket.send_to(&packet, self.to_addr));
        Ok(())
    }

}

pub fn broadcast_addr() -> net::SocketAddr {
    use std::net::*;
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)), PORT)
}

pub fn discover(timeout: time::Duration) -> io::Result<Vec<net::SocketAddr>> {
    let socket = try!(net::UdpSocket::bind(("0.0.0.0", PORT)));
    try!(socket.set_broadcast(true));

    // Send out an ArtPoll packet to elicit an ArtPollReply from all devices in the network.
    let mut buf = Vec::new();
    try!(art_poll_packet(&mut buf));
    try!(socket.send_to(&buf, ("255.255.255.255", PORT)));

    try!(socket.set_read_timeout(Some(timeout)));
    let mut sockets = Vec::new();
    loop {
        let mut recv_buf = [0; 168];
        let (_, sender_addr) = match socket.recv_from(&mut recv_buf) {
            Err(_) => break,
            Ok(rs) => rs,
        };
        if &recv_buf[0..8] != b"Art-Net\0" {
            continue;
        }
        let mut rdr = io::Cursor::new(&recv_buf[8..10]);
        let opcode = try!(rdr.read_u16::<LittleEndian>());
        if opcode == 0x2100 {
            sockets.push(sender_addr);
        }
    }
    Ok(sockets)
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
