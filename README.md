# Serialxy
Serial port proxy, a proxy tool can help you to connect remote serial port like telnet.

* Support Linux, Windows
* mio-only version, less dependencies, less binary size

## install

```bash
$ cargo install serialxy
```

## example

* host
```bash
$ serialxy /dev/ttyUSB1,115200
```

* client
```bash
$ telnet remote-ip 8722
^]
telnet> mode character
```

* Then you can use telnet connect to your serial port as same as using serial port directly.

![example](example.png)
