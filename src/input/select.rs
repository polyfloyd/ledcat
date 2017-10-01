use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::path;
use libc;
use nix::poll;


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WhenEOF {
    Close,
    Retry,
}

pub trait ReadFd: io::Read + AsRawFd { }

impl<T> ReadFd for T
    where T: io::Read + AsRawFd { }

pub struct Reader {
    when_eof: WhenEOF,

    inputs: Vec<Box<ReadFd + Send>>,
    // The number of bytes after which another input is selected.
    switch_after: usize,
    // A buffer for each input to be used for partially received content.
    buffers: Vec<Vec<u8>>,
    // The current buffer selected for output.
    current: io::Cursor<Vec<u8>>,
}

impl Reader {
    pub fn from_files<P>(filenames: Vec<P>, switch_after: usize, when_eof: WhenEOF) -> io::Result<Reader>
        where P: AsRef<path::Path> {
        let files: io::Result<Vec<Box<ReadFd + Send>>> = filenames.into_iter().map(|filename| {
            let mut open_opts = fs::OpenOptions::new();
            open_opts.read(true);

            let is_fifo = fs::metadata(&filename)?.file_type().is_fifo();
            if is_fifo {
                // A FIFO will block the call to open() until the other end has been opened. This
                // means that when multiple FIFO's are used, they all have to be open at once
                // before this program can continue.
                // Opening the file with O_NONBLOCK will ensure that we don't have to wait.
                open_opts.custom_flags(libc::O_NONBLOCK);
            }

            let file = open_opts.open(&filename)?;

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

            Ok(Box::<ReadFd + Send>::from(Box::new(file)))
        }).collect();
        Ok(Reader::from(files?, switch_after, when_eof))
    }

    pub fn from(inputs: Vec<Box<ReadFd + Send>>, switch_after: usize, when_eof: WhenEOF) -> Reader {
        assert_ne!(inputs.len(), 0);
        let buffers = (0..inputs.len())
            .map(|_| Vec::with_capacity(switch_after))
            .collect();
        Reader {
            switch_after,
            buffers,
            when_eof,
            inputs,
            current: io::Cursor::new(Vec::new()),
        }
    }
}

impl io::Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.current.position() == self.current.get_ref().len() as u64 {
            // The end of the current buffer has been reached, fetch more data.
            loop {
                // Perform a poll to see if there are any inputs ready for reading.
                let mut poll_fds: Vec<_> = self.inputs.iter()
                    .map(|inp| {
                        poll::PollFd::new(inp.as_raw_fd(), poll::POLLIN)
                    })
                    .collect();
                io_err!(poll::poll(&mut poll_fds, 1_000))?;

                let mut num_open = poll_fds.len();
                let mut ready_index = None;
                for (i, p) in poll_fds.iter().enumerate() {
                    let rev = p.revents().unwrap();
                    if rev.contains(poll::POLLIN) {
                        let buf = &mut self.buffers[i];
                        let buf_used = buf.len();
                        assert_ne!(buf_used, self.switch_after);
                        // Resize the buffer so there is just enough space for the remainder of the
                        // frame.
                        buf.resize(self.switch_after, 0);

                        let nread = self.inputs[i].read(&mut buf[buf_used..])?;
                        buf.resize(buf_used + nread, 0);
                        assert!(buf.len() <= self.switch_after);
                        if nread == 0 { // EOF
                            num_open -= 1;
                        } else if buf.len() == self.switch_after {
                            ready_index = Some(i);
                            break;
                        }
                    } else if rev.intersects(poll::POLLHUP|poll::POLLNVAL|poll::POLLERR) {
                        num_open -= 1;
                    }
                }

                if num_open == 0 && self.when_eof == WhenEOF::Close {
                    return Ok(0);
                }

                if let Some(i) = ready_index {
                    let tail = self.buffers[i].split_off(self.switch_after);
                    self.buffers.push(tail); // Later moved to index i by swap_remove.
                    let buf = self.buffers.swap_remove(i);
                    self.current = io::Cursor::new(buf);
                    break;
                }
            }
        }
        self.current.read(buf)
    }
}


#[cfg(test)]
mod tests {
    extern crate rand;
    extern crate tempdir;
    use std::*;
    use std::io::{Seek, Read, Write};
    use std::os::unix::io::FromRawFd;
    use std::sync::mpsc;
    use nix::sys::memfd::*;
    use super::*;
    use self::rand::Rng;

    macro_rules! timeout {
        ($timeout:expr, $block:block) => {
            let (tx, rx) = mpsc::sync_channel(1);
            thread::spawn(move || {
                $block;
                let _ = tx.send(());
            });
            if let Err(_) = rx.recv_timeout($timeout) {
                panic!("Timeout expired");
            }
        }
    }

    fn new_iter_reader<I>(iter: I) -> Box<fs::File>
        where I: iter::Iterator<Item = u8> {
        let name = rand::thread_rng().gen_ascii_chars()
            .take(32)
            .collect::<String>();
        let cname = ffi::CString::new(name).unwrap();
        let fd = memfd_create(&cname, MemFdCreateFlag::empty()).unwrap();
        let mut f = unsafe { fs::File::from_raw_fd(fd) };
        for b in iter {
            f.write_all(&[b]).unwrap();
        }
        f.seek(io::SeekFrom::Start(0)).unwrap();
        Box::new(f)
    }

    fn copy_iter<I: iter::Iterator<Item = u8>>(wr: &mut io::Write, it: I) {
        let v: Vec<u8> = it.collect();
        wr.write(&v).unwrap();
        wr.flush().unwrap();
    }

    #[test]
    fn read_one_input() {
        let len = 100;
        let num = 16;
        let testdata: Vec<u8> = (1..num + 1)
            .fold(Box::from(iter::empty()) as Box<iter::Iterator<Item = _>>,
                  |ch, i| Box::from(ch.chain(iter::repeat(i as u8).take(len))))
            .collect();

        let mut reader = Reader::from(
            vec![new_iter_reader(testdata.clone().into_iter())],
            len,
            WhenEOF::Close,
        );

        for i in 0..num {
            let mut rd_buf = vec![0; len];
            reader.read_exact(&mut rd_buf).unwrap();
            assert_eq!(testdata[len * i..len * (i + 1)], rd_buf[..]);
        }
        timeout!(time::Duration::new(1, 0), {
            assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
        });
    }

    #[test]
    fn read_multiple_inputs_order() {
        let len = 100;
        let num = 16;

        let mut reader = Reader::from(
            (1..num + 1).map(|i| new_iter_reader(iter::repeat(i).take(len)) as Box<ReadFd + Send>).collect(),
            len,
            WhenEOF::Close,
        );

        for i in 1..num + 1 {
            let mut rd_buf = vec![0; len];
            reader.read_exact(&mut rd_buf).unwrap();
            let expected: Vec<u8> = iter::repeat(i).take(len).collect();
            assert_eq!(expected, rd_buf);
        }
        timeout!(time::Duration::new(1, 0), {
            assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
        });
    }

    #[test]
    fn read_eof() {
        let mut reader = Reader::from(
            vec![new_iter_reader(iter::empty()), new_iter_reader(iter::empty())],
            1,
            WhenEOF::Close,
        );
        timeout!(time::Duration::new(1, 0), {
            assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
        });
    }

    #[test]
    #[should_panic]
    fn read_eof_retry() {
        let mut reader = Reader::from(
            vec![new_iter_reader(iter::empty())],
            1,
            WhenEOF::Retry,
        );
        timeout!(time::Duration::new(0, 100_000_000), {
            io::copy(&mut reader, &mut io::sink()).unwrap();
        });
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

        let mut reader = Reader::from_files(
            vec![&fifo1_path, &fifo2_path],
            len,
            WhenEOF::Close,
        ).unwrap();
        let mut fifo1 = fs::OpenOptions::new().write(true).open(&fifo1_path).unwrap();
        let mut fifo2 = fs::OpenOptions::new().write(true).open(&fifo2_path).unwrap();

        let mut rd_buf = vec![0; len];

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
        timeout!(time::Duration::new(1, 0), {
            assert_eq!(0, io::copy(&mut reader, &mut io::sink()).unwrap());
        });

        tmp.close().unwrap();
    }
}
