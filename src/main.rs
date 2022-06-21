use exchange::exchange;
use mio::net::TcpStream;
use mio::{net::TcpListener, Token};
use mio::{Events, Interest, Poll};
use mio_serial::SerialPortBuilderExt;
use std::env;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;

mod exchange;
struct SerialConfig {
    pub name: String,
    pub baudrate: u32,
}

impl SerialConfig {
    fn form_str(serial_desc: &str) -> std::result::Result<SerialConfig, &str> {
        match serial_desc.split_once(',') {
            Some((name, baudrate)) => match baudrate.parse::<u32>() {
                Err(_e) => {
                    return Err("invaild baudrate");
                }
                Ok(baudrate) => {
                    return Ok(SerialConfig {
                        name: name.into(),
                        baudrate,
                    });
                }
            },
            None => {
                return Ok(SerialConfig {
                    name: serial_desc.into(),
                    baudrate: 115200,
                })
            }
        }
    }
}

const SERVER: Token = Token(0);

fn start_server(
    ipaddr: SocketAddr,
    serial_cfg: SerialConfig,
    buffer_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a poll instance.
    let mut poll = Poll::new()?;
    // Create storage for events.
    let mut events = Events::with_capacity(128);

    let mut server = TcpListener::bind(ipaddr)?;

    println!("Server on {}", ipaddr);
    // Start listening for incoming connections.
    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)?;

    // Start an event loop.
    loop {
        // Poll Mio for events, blocking until we get an event.
        poll.poll(&mut events, None)?;

        // Process each event.
        for event in events.iter() {
            // We can use the token we previously provided to `register` to
            // determine for which socket the event is.
            match event.token() {
                SERVER => {
                    // If this is an event for the server, it means a connection
                    // is ready to be accepted.
                    //
                    // Accept the connection and drop it immediately. This will
                    // close the socket and notify the client of the EOF.
                    let (socket, addr) = server.accept()?;
                    poll.registry().deregister(&mut server)?;
                    println!("Connect: {}", addr);

                    let serial = mio_serial::new(serial_cfg.name.as_str(), serial_cfg.baudrate)
                        .open_native_async()?;
                    exchange::exchange(socket, serial, buffer_size)?;
                    println!("Disconnect: {}", addr);
                    poll.registry()
                        .register(&mut server, SERVER, Interest::READABLE)?;
                }
                // We don't expect any events with tokens other than those we provided.
                _ => unreachable!(),
            }
        }
    }
}

fn print_usage(program: &str) {
    let help_info = r#"
    serial-name:    like 'COM1,115200' or '/dev/ttyUSB0', the default baudrate is 115200
    -c              client mode, forward data to local serial-port
    -p              specific server-port, the default port is 8722
    -b              buffer size, 512 bytes by default
    -h              help
"#;
    print!("Usage: {} serial-name [ options ]{}", program, help_info);
}

fn main() {
    let mut args = env::args();
    let mut remote_ip: Option<SocketAddr> = None;
    let mut server_port = 8722;
    let mut serial_cfg: Option<SerialConfig> = None;
    let mut buffer_size = 512;
    let program = args.next().unwrap();
    loop {
        match args.next() {
            Some(arg) => match arg.as_str() {
                "-p" => match args.next() {
                    Some(port) => match port.parse::<u16>() {
                        Ok(port) => {
                            server_port = port;
                        }
                        Err(e) => {
                            println!("error: {}", e);
                            return;
                        }
                    },
                    None => {
                        println!("error: please specific port number");
                        return;
                    }
                },
                "-c" => match args.next() {
                    Some(addr) => match SocketAddr::from_str(addr.as_str()) {
                        Ok(addr) => {
                            remote_ip = Some(addr);
                        }
                        Err(e) => {
                            println!("error: {}", e);
                            return;
                        }
                    },
                    None => {
                        println!("error: please specific remote ip");
                        return;
                    }
                },
                "-b" => match args.next() {
                    Some(buff_size) => {
                        buffer_size = match buff_size.parse::<usize>() {
                            Ok(buffer_size) => {
                                if buffer_size < 512 {
                                    println!("warning: buffer size should greater than 512 bytes");
                                    512
                                } else {
                                    buffer_size
                                }
                            }
                            Err(e) => {
                                println!("error: {}", e);
                                return;
                            }
                        };
                    }
                    None => {
                        println!("error: please specific port number");
                        return;
                    }
                },
                "-b" => match args.next() {
                    Some(size) => {
                        buffer_size = match size.parse::<usize>() {
                            Ok(buffer_size) => {
                                if buffer_size < 512 {
                                    println!("warning: buffer size should more than 512 bytes");
                                    512
                                } else {
                                    buffer_size
                                }
                            }
                            Err(e) => {
                                println!("error: {}", e);
                                return Ok(());
                            }
                        };
                    }
                    None => {
                        println!("error: please specific buffer size");
                        return Ok(());
                    }
                },

                "-h" => {
                    print_usage(&program);
                    return;
                }

                serial_desc => match serial_cfg {
                    Some(_) => {
                        return;
                    }
                    None => match SerialConfig::form_str(serial_desc) {
                        Ok(cfg) => {
                            serial_cfg = Some(cfg);
                        }
                        Err(e) => {
                            println!("error: {}", &e);
                        }
                    },
                },
            },
            None => {
                break;
            }
        }
    }

    match serial_cfg {
        None => {
            println!("error: no serial port specified! Try '-h' for more information");
            return;
        }
        Some(serial_cfg) => match remote_ip {
            Some(ipaddr) => match TcpStream::connect(ipaddr) {
                Ok(socket) => {
                    match mio_serial::new(serial_cfg.name.as_str(), serial_cfg.baudrate)
                        .open_native_async()
                    {
                        Err(e) => {
                            println!(
                                "open serial port {}, baudrate = {} failed, {}",
                                &serial_cfg.name, &serial_cfg.baudrate, e.description
                            );
                        }
                        Ok(serial) => match exchange(socket, serial, buffer_size) {
                            Err(e) => {
                                println!("error: {}", e.to_string());
                            }
                            Ok(_) => {
                                return;
                            }
                        },
                    }
                }
                Err(e) => {
                    println!("error: {}", e.to_string());
                }
            },
            None => match ("0.0.0.0", server_port).to_socket_addrs() {
                Ok(mut addrs) => match addrs.next() {
                    None => {
                        println!("error: invaild ip address");
                    }
                    Some(ipaddr) => match start_server(ipaddr, serial_cfg, buffer_size) {
                        Err(e) => {
                            println!("error: {}", e.to_string());
                        }
                        Ok(_) => {}
                    },
                },
                Err(e) => {
                    println!("error: {}", e);
                }
            },
        },
    }
}
