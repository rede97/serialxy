use std::env;
use std::io::Cursor;
use std::io::Result;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_serial;
use tokio_serial::SerialPortBuilderExt;
use tokio_serial::SerialStream;

struct SerialConfig {
    name: String,
    baudrate: u32,
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

async fn exchange(
    mut socket: TcpStream,
    mut serial: SerialStream,
) -> std::result::Result<(), String> {
    let mut socket_rx_buffer: [u8; 128] = [0; 128];
    let mut serial_rx_buffer: [u8; 128] = [0; 128];
    loop {
        tokio::select! {
            socket_nread = socket.read(&mut socket_rx_buffer) => {
                match socket_nread {
                    Ok(nread) => {
                        if nread == 0 {
                            break;
                        } else {
                            match serial.write(&socket_rx_buffer[0..nread]).await {
                                Ok(_) => {}
                                Err(e) => {
                                    return Err(e.to_string());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return Err(e.to_string());
                    }
                }
            }

            serial_nread = serial.read(&mut serial_rx_buffer) => {
                match serial_nread {
                    Ok(nread) => {
                        let mut cursor = Cursor::new(&serial_rx_buffer[0..nread]);
                        match socket.write_buf(&mut cursor).await {
                            Ok(nwrite) => {
                                if nwrite == 0 {
                                    break;
                                }
                            }
                            Err(e) => {
                                return Err(e.to_string());
                            }
                        }
                    }
                    Err(e) => {
                        return Err(e.to_string());
                    }
                }
            }
        }
    }
    Ok(())
}

async fn start_server(ip: SocketAddr, serial_cfg: SerialConfig) -> std::result::Result<(), String> {
    let listener = match TcpListener::bind(ip).await {
        Ok(l) => l,
        Err(e) => {
            return Err(format!("listen on {} failed, {}", ip.port(), e.to_string()));
        }
    };
    println!("Server on {}", listener.local_addr().unwrap());
    loop {
        match listener.accept().await {
            Ok((socket, client_addr)) => {
                println!("Accept {}", client_addr);
                match tokio_serial::new(&serial_cfg.name, serial_cfg.baudrate).open_native_async() {
                    Ok(serial) => match exchange(socket, serial).await {
                        Ok(_) => {
                            println!("Disconnect {}", client_addr);
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    Err(e) => {
                        return Err(format!(
                            "open serial port {}, baudrate = {} failed, {}",
                            &serial_cfg.name, &serial_cfg.baudrate, e.description
                        ));
                    }
                };
            }
            Err(e) => {
                println!("warning: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args();
    let mut remote_ip: Option<SocketAddr> = None;
    let mut server_port = 8722;
    let mut serial_cfg: Option<SerialConfig> = None;
    args.next();
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
                            return Ok(());
                        }
                    },
                    None => {
                        println!("error: please specific port number");
                        return Ok(());
                    }
                },
                "-c" => match args.next() {
                    Some(addr) => match SocketAddr::from_str(addr.as_str()) {
                        Ok(addr) => {
                            remote_ip = Some(addr);
                        }
                        Err(e) => {
                            println!("error: {}", e);
                            return Ok(());
                        }
                    },
                    None => {
                        println!("error: please specific remote ip");
                        return Ok(());
                    }
                },

                "-h" => {
                    let usage = "Usage: serialxy seiral-name [-c remote-ip] [-p port]\n\tserial-name: like COM1,115200 /dev/ttyUSB0, the default baudrate if 115200\n\t-c\tclient mode, forward data to local serial-port\n\t-p\tspecific server-port, the default port is 8722\n\t-h\thelp\n";
                    print!("{}", usage);
                    return Ok(());
                }

                serial_desc => match serial_cfg {
                    Some(_) => {
                        return Ok(());
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
            println!("error: no serial port specified!");
            return Ok(());
        }
        Some(serial_cfg) => match remote_ip {
            Some(remote_ip) => match TcpStream::connect(remote_ip).await {
                Err(e) => {
                    println!("error: {}", e.to_string());
                }
                Ok(socket) => {
                    match tokio_serial::new(&serial_cfg.name, serial_cfg.baudrate)
                        .open_native_async()
                    {
                        Ok(serial) => match exchange(socket, serial).await {
                            Ok(_) => {
                                println!("Disconnect {}", remote_ip);
                            }
                            Err(e) => {
                                println!("error: {}", e);
                            }
                        },
                        Err(e) => {
                            println!(
                                "open serial port {}, baudrate = {} failed, {}",
                                &serial_cfg.name, &serial_cfg.baudrate, e.description
                            );
                        }
                    };
                    return Ok(());
                }
            },
            None => {
                match ("0.0.0.0", server_port).to_socket_addrs() {
                    Ok(mut ips) => match ips.next() {
                        Some(ip) => match start_server(ip, serial_cfg).await {
                            Err(e) => {
                                println!("error: {}", e);
                            }
                            Ok(_) => {}
                        },
                        None => {
                            println!("error: invaild ip address");
                            return Ok(());
                        }
                    },
                    Err(e) => {
                        println!("error: {}", e);
                        return Ok(());
                    }
                };
            }
        },
    }

    return Ok(());
}
