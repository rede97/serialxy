use mio::{net::TcpStream, Events, Interest, Poll, Token};
use mio_serial::SerialStream;
use std::io;
use std::io::{Read, Write};

const SOCKET: Token = Token(0);
const SERIAL: Token = Token(1);
const BUFFER_SIZE: usize = 512;

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}

struct Buffer {
    buff: Vec<u8>,
    idx: usize,
}

impl Buffer {
    pub fn new(cap: usize) -> Self {
        return Buffer {
            buff: vec![0; cap],
            idx: 0,
        };
    }

    pub fn free_size(&self) -> usize {
        return self.buff.len() - self.idx;
    }

    pub fn free(&mut self) -> &mut [u8] {
        return &mut self.buff[self.idx..];
    }

    pub fn avaliable(&mut self, nread: usize) -> &[u8] {
        self.idx += nread;
        return &self.buff[..self.idx];
    }

    pub fn update(&mut self, nwrite: usize) {
        if nwrite != self.idx {
            // unlikely
            if nwrite != 0 {
                // move avaliable data to head
                for i in 0..self.idx - nwrite {
                    self.buff[i] = self.buff[nwrite + i];
                }
                self.idx -= nwrite;
            }
        } else {
            self.idx = 0;
        }
    }
}

pub fn exchange(
    mut socket: TcpStream,
    mut serial: SerialStream,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(32);
    
    #[cfg(unix)]
    serial
        .set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    let mut socket_rx_buffer = Buffer::new(BUFFER_SIZE);
    let mut serial_rx_buffer = Buffer::new(BUFFER_SIZE);

    poll.registry()
        .register(&mut socket, SOCKET, Interest::READABLE)?;
    poll.registry()
        .register(&mut serial, SERIAL, Interest::READABLE)?;

    'event_loop: loop {
        poll.poll(&mut events, None)?;
        for event in events.iter() {
            match event.token() {
                SOCKET => {
                    if event.is_readable() {
                        poll.registry().reregister(
                            &mut serial,
                            SERIAL,
                            Interest::READABLE | Interest::WRITABLE,
                        )?;
                    }
                    if event.is_writable() {
                        loop {
                            match serial.read(serial_rx_buffer.free()) {
                                Ok(nread) => {
                                    // The serial_rx_buffer avaliable size must be not zero
                                    match socket.write(serial_rx_buffer.avaliable(nread)) {
                                        Ok(0) => {
                                            break 'event_loop;
                                        }
                                        Ok(nwrite) => {
                                            serial_rx_buffer.update(nwrite);
                                        }
                                        Err(ref err) if would_block(err) => {
                                            break;
                                        }
                                        Err(ref err) if interrupted(err) => continue,
                                        // Other errors we'll consider fatal.
                                        Err(err) => return Err(Box::new(err)),
                                    }
                                }
                                Err(ref err) if would_block(err) => {
                                    poll.registry().reregister(
                                        &mut socket,
                                        SOCKET,
                                        Interest::READABLE,
                                    )?;
                                    break;
                                }
                                Err(e) => {
                                    return Err(Box::new(e));
                                }
                            }
                        }
                    }

                    if event.is_read_closed() || event.is_write_closed() {
                        break 'event_loop;
                    }
                }
                SERIAL => {
                    if event.is_readable() {
                        poll.registry().reregister(
                            &mut socket,
                            SOCKET,
                            Interest::READABLE | Interest::WRITABLE,
                        )?;
                    }

                    if event.is_writable() {
                        loop {
                            match socket.read(socket_rx_buffer.free()) {
                                Ok(nread) => {
                                    if nread == 0 && socket_rx_buffer.free_size() != 0 {
                                        break 'event_loop;
                                    }
                                    match serial.write(socket_rx_buffer.avaliable(nread)) {
                                        Ok(nwrite) => {
                                            socket_rx_buffer.update(nwrite);
                                        }
                                        Err(ref err) if would_block(err) => {
                                            break;
                                        }
                                        Err(err) => {
                                            return Err(Box::new(err));
                                        }
                                    }
                                }
                                Err(ref err) if would_block(err) => {
                                    poll.registry().reregister(
                                        &mut serial,
                                        SERIAL,
                                        Interest::READABLE,
                                    )?;
                                    break;
                                }
                                Err(ref err) if interrupted(err) => continue,
                                // Other errors we'll consider fatal.
                                Err(err) => return Err(Box::new(err)),
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    return Ok(());
}
