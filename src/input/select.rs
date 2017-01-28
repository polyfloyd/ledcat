use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path;
use std::sync::mpsc;
use std::thread;
use std::time;
use libc;

/// Tells readers how to handle excessive data if more is produced from the inputs than is being
/// read.
#[derive(Clone)]
pub enum Consume {
    /// Only pull data from the selected input. All other streams are blocked and their data is
    /// kept until needed.
    Single,
    /// Keeps pulling from all streams regardless of whether the data is actually being presented
    /// to the reader. Be sure to idle between frames in the producer to make sure no frames are
    /// dropped.
    All(time::Duration),
}

#[derive(Clone)]
pub enum WhenEOF {
    Close,
    Retry,
}


pub struct Reader {
    consume:   Consume,
    when_eof:  WhenEOF,
    receivers: Vec<mpsc::Receiver<Vec<u8>>>,
    current:   Option<io::Cursor<Vec<u8>>>,
}

impl Reader {

    pub fn from_files<P>(filenames: Vec<P>, switch_after: usize, consume: Consume, when_eof: WhenEOF) -> io::Result<Reader>
        where P: AsRef<path::Path> {
        let files: io::Result<Vec<fs::File>> = filenames.into_iter().map(|filename| {
            let mut open_opts = fs::OpenOptions::new();
            open_opts.read(true);

            let is_fifo = cfg!(unix) && try!(fs::metadata(&filename)).file_type().is_fifo();
            if is_fifo {
                // A FIFO will block the call to open() until the other end has been opened. This
                // means that when multiple FIFO's are used, they all have to be open at once
                // before this program can continue.
                // Opening the file with O_NONBLOCK will ensure that we don't have to wait.
                open_opts.custom_flags(libc::O_NONBLOCK);
            }

            let file = try!(open_opts.open(&filename));

            if is_fifo {
                unsafe {
                    // Now unset the O_NONBLOCK flag so reads will block again.
                    let fd = file.as_raw_fd();
                    let opts = libc::fcntl(fd, libc::F_GETFL);
                    assert!(opts & libc::O_NONBLOCK > 0);
                    if opts < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    if libc::fcntl(fd, libc::F_SETFL, opts & !libc::O_NONBLOCK) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
            }

            Ok(file)
        }).collect();
        Ok(Reader::from(try!(files), switch_after, consume, when_eof))
    }

    pub fn from<R>(inputs: Vec<R>, switch_after: usize, consume: Consume, when_eof: WhenEOF) -> Reader
        where R: io::Read + Send + 'static {
        assert_ne!(inputs.len(), 0);

        let receivers = inputs.into_iter().map(|mut input| {
            let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
            let consume = consume.clone();
            let when_eof = when_eof.clone();
            thread::spawn(move || {
                loop {
                    let mut buf = Vec::new();
                    buf.resize(switch_after, 0);
                    if let Err(_) = input.read_exact(&mut buf) {
                        if let WhenEOF::Close = when_eof {
                            return;
                        }
                        thread::sleep(time::Duration::new(0, 1_000_000)); // TODO
                        continue;
                    }
                    match consume {
                        Consume::Single => {
                            if let Err(_) = tx.send(buf) {
                                break;
                            }
                        },
                        Consume::All(interval) => {
                            if let Err(mpsc::TrySendError::Disconnected(_)) = tx.try_send(buf) {
                                break;
                            }
                            thread::sleep(interval);
                        },
                    };
                }
            });
            rx
        }).collect();

        Reader {
            consume:   consume,
            when_eof:  when_eof,
            receivers: receivers,
            current:   None,
        }
    }

}

impl io::Read for Reader {

    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.current.is_none() {
                let recv = self.receivers.iter().fold(Err(false), |prev, rx| {
                    let prev_active = match prev {
                        Ok(_)       => true,
                        Err(active) => active,
                    };
                    let recv = match self.consume {
                        Consume::Single => prev.or_else(|_| rx.try_recv()),
                        Consume::All(_) => prev.or(rx.try_recv()),
                    };
                    recv.map_err(|e| match e {
                        mpsc::TryRecvError::Disconnected => prev_active,
                        mpsc::TryRecvError::Empty        => true,
                    })
                });
                if let Err(false) = recv {
                    if let WhenEOF::Close = self.when_eof {
                        return Ok(0);
                    }
                }
                self.current = recv.ok().map(io::Cursor::new);
            }

            if self.current.is_some() {
                let mut cur = self.current.take().unwrap();
                let nread = cur.read(&mut buf).unwrap();
                self.current = if nread == 0 { None } else { Some(cur) };
                if nread > 0 {
                    return Ok(nread);
                }
            }

            thread::sleep(time::Duration::new(0, 1_000_000)); // TODO
        }
    }

}


#[cfg(test)]
mod tests {
    extern crate tempdir;
    use std::*;
    use std::io::Read;
    use std::sync::mpsc;
    use super::*;

    struct IterReader<I: iter::Iterator<Item=u8>>(I);

    impl<I> io::Read for IterReader<I>
        where I: iter::Iterator<Item=u8> {
        fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
            for i in 0..buf.len() {
                match self.0.next() {
                    Some(b) => buf[i] = b,
                    None    => return Ok(i),
                };
            }
            Ok(buf.len())
        }
    }

    fn copy_iter<I: iter::Iterator<Item=u8>>(wr: &mut io::Write, it: I) {
        let v: Vec<u8> = it.collect();
        wr.write(&v).unwrap();
        wr.flush().unwrap();
    }

    #[test]
    fn read_one_input() {
        let len = 100;
        let num = 16;
        let testdata: Vec<u8> = (1..num+1)
            .fold(Box::from(iter::empty()) as Box<iter::Iterator<Item=_>>, |ch, i| {
                Box::from(ch.chain(iter::repeat(i as u8).take(len)))
            }).collect();

        let mut reader = Reader::from(vec![
            io::Cursor::new(testdata.clone()),
        ], len, Consume::Single, WhenEOF::Close);

        for i in 0..num {
            let mut rd_buf = Vec::new();
            rd_buf.resize(len, 0);
            reader.read_exact(&mut rd_buf).unwrap();
            assert_eq!(testdata[len * i..len * (i + 1)], rd_buf[..]);
        }
        assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
    }

    #[test]
    fn read_multiple_inputs_order() {
        let len = 100;
        let num = 16;

        let mut reader = Reader::from((1..num+1).map(|i| {
            IterReader(iter::repeat(i).take(len))
        }).collect(), len, Consume::Single, WhenEOF::Close);

        for i in 1..num + 1 {
            let mut rd_buf = Vec::new();
            rd_buf.resize(len, 0);
            reader.read_exact(&mut rd_buf).unwrap();
            let expected: Vec<u8> = iter::repeat(i).take(len).collect();
            assert_eq!(expected, rd_buf);
        }
        assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
    }

    #[test]
    fn read_eof() {
        let mut reader = Reader::from(vec![
            IterReader(iter::empty()),
            IterReader(iter::empty()),
        ], 1, Consume::Single, WhenEOF::Close);
        assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
    }

    #[test]
    fn read_switching() {
        let len = 100;
        let (pat1, pat2) = (12, 42);

        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let mut reader = Reader::from(vec![
            IterReader(rx1.into_iter()),
            IterReader(rx2.into_iter()),
        ], len, Consume::Single, WhenEOF::Close);

        let mut rd_buf = Vec::new();
        rd_buf.resize(len, 0);

        // Send a partial frame over channel 1...
        for v in iter::repeat(pat1).take(len - 1) { tx1.send(v).unwrap(); }

        // Send and receive a full frame over channel 2.
        let testdata: Vec<u8> = iter::repeat(pat2).take(len).collect();
        for v in testdata.clone().into_iter() { tx2.send(v).unwrap(); }
        reader.read_exact(&mut rd_buf).unwrap();
        assert_eq!(testdata, rd_buf);
        rd_buf.resize(len, 0);

        // ...and complete that first frame over channel 1.
        tx1.send(pat1).unwrap();
        reader.read_exact(&mut rd_buf).unwrap();
        let expected: Vec<u8> = iter::repeat(pat1).take(len).collect();
        assert_eq!(expected, rd_buf);

        drop(tx1);
        drop(tx2);
        assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn read_unix_fifo() {
        use libc;

        let len = 10;
        let (pat1, pat2) = (12, 42);

        let tmp = tempdir::TempDir::new("read_unix_fifo").unwrap();
        let fifo1_path = tmp.path().join("fifo1").into_os_string();
        let fifo2_path = tmp.path().join("fifo2").into_os_string();
        unsafe {
            let f1 = ffi::CString::new(fifo1_path.to_str().unwrap()).unwrap();
            let f2 = ffi::CString::new(fifo2_path.to_str().unwrap()).unwrap();
            assert_eq!(0, libc::mkfifo(f1.as_ptr(), 0o666));
            assert_eq!(0, libc::mkfifo(f2.as_ptr(), 0o666));
        }

        let mut reader = Reader::from_files(vec![ &fifo1_path, &fifo2_path ], len, Consume::Single, WhenEOF::Close).unwrap();
        let mut fifo1 = fs::OpenOptions::new().write(true).open(&fifo1_path).unwrap();
        let mut fifo2 = fs::OpenOptions::new().write(true).open(&fifo2_path).unwrap();

        let mut rd_buf = Vec::new();
        rd_buf.resize(len, 0);

        // Send a partial frame over fifo 1...
        copy_iter(&mut fifo1, iter::repeat(pat1).take(len - 1));

        // Send and receive a full frame over fifo 2.
        let testdata: Vec<u8> = iter::repeat(pat2).take(len).collect();
        copy_iter(&mut fifo2, testdata.clone().into_iter());
        reader.read_exact(&mut rd_buf).unwrap();
        assert_eq!(testdata, rd_buf);
        rd_buf.resize(len, 0);

        // ...and complete that first frame over fifo 1.
        copy_iter(&mut fifo1, iter::once(pat1));
        reader.read_exact(&mut rd_buf).unwrap();
        let expected: Vec<u8> = iter::repeat(pat1).take(len).collect();
        assert_eq!(expected, rd_buf);

        drop(fifo1);
        drop(fifo2);
        assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());

        tmp.close().unwrap();
    }
}
