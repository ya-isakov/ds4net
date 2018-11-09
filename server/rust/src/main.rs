use std::thread;
use std::net::UdpSocket;
use std::fs::File;
use std::io::prelude::*;
use std::net::SocketAddr;

extern crate crc;
use crc::crc32;

//static mut clients: Vec<SocketAddr> = Vec::new();

fn handle_new_client(addr: SocketAddr, fname: String) {
    println!("addr: {}", addr);
    let mut f = File::open(fname).expect("file problem");
    let client = UdpSocket::bind("0.0.0.0:0").expect("azaza");
    client.connect(addr).expect("ababa");
    let mut connected = false;
    loop {
        let mut packet = [0; 77];
        let count = f.read(&mut packet).expect("oops");
        assert_eq!(count, 77);
        assert_eq!(packet[0], 0x11);
        match client.send(&packet) {
            Ok(_) => true,
            Err(ref err) if (err.kind() == std::io::ErrorKind::ConnectionRefused && connected) => {
                println!("{} disconnected", addr);
                break;
            },
            Err(err) => {
                println!("Error on address {} {}", addr, err);
                break;
            }
        };
        connected = true;
    }
}

fn handle_disconnect(addr: SocketAddr, _clients: &mut Vec<SocketAddr>) {
    
}

fn handle_rumble() {
}

//fn is_sync<T: Send>() {}

fn main() {

    //is_sync::<*mut Vec<std::net::SocketAddr>>();
    /*let hasher = crc32::checksum_ieee(b"1234");
    println!("hash {}", hasher.to_be());*/
    //let mut clients = Vec::new();
    let socket = UdpSocket::bind("127.0.0.1:9999").expect("bla-bla");

    // Receives a single datagram message on the socket. If `buf` is too small to hold
    // the message, it will be cut off.
    let mut buf = [0; 4];
    loop {
        let (amt, src) = socket.recv_from(&mut buf).expect("fooo!");
        //clients.push(src);
        // Redeclare `buf` as slice of the received data and send reverse data back to origin.
        let buf = &mut buf[..amt];
        let mut handle;
        match buf[0] {
            0 => handle = thread::spawn( move || { handle_new_client(src, "/dev/hidraw2".to_string()) } ),
            1 => handle_rumble(),
            //2 => handle_disconnect(src, &mut clients),
            _ => panic!("Bohuzhel"),
        };
        //println!("amt: {:?}", clients);
        //println!("src: {}", src);
        //println!("src: {:?}", _buf);
        //buf.reverse();
        //socket.send_to(buf, &src)?;
    }
    //handle.join().unwrap();
}
