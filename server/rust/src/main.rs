use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use std::thread;

extern crate crossbeam_channel;
use crossbeam_channel::{unbounded, Receiver, Sender};

extern crate crc;
use crc::crc32;

fn handle_new_client(addr: SocketAddr, r: Receiver<[u8; 77]>) {
    println!("addr: {}", addr);
    let client = UdpSocket::bind("0.0.0.0:0").unwrap();
    client.connect(addr).unwrap();
    let mut connected = false;
    loop {
        let packet = r.recv().unwrap();
        match client.send(&packet) {
            Ok(_) => true,
            Err(ref err) if (err.kind() == std::io::ErrorKind::ConnectionRefused && connected) => {
                println!("{} disconnected", addr);
                break;
            }
            Err(err) => {
                println!("Error on address {} {}", addr, err);
                break;
            }
        };
        connected = true;
    }
}

fn handle_disconnect(addr: SocketAddr, _clients: &mut Vec<SocketAddr>) {}

fn handle_rumble() {}

fn handle_udp(clients: Arc<RwLock<HashMap<SocketAddr, Sender<[u8; 77]>>>>) {
    let socket = UdpSocket::bind("127.0.0.1:9999").unwrap();
    let mut buf = [0; 4];
    loop {
        let (amt, src) = socket.recv_from(&mut buf).unwrap();
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let (s, r) = unbounded();
                clients.write().unwrap().insert(src, s);
                let handle = thread::spawn(move || handle_new_client(src, r));
            }
            1 => handle_rumble(),
            _ => panic!("Bohuzel"),
        };
    }
}

fn main() {
    //is_sync::<*mut Vec<std::net::SocketAddr>>();
    /*let hasher = crc32::checksum_ieee(b"1234");
    println!("hash {}", hasher.to_be());*/
    let mut clients = Arc::new(RwLock::new(HashMap::new()));
    let c_clients = Arc::clone(&clients);
    let handle = thread::spawn(move || {
        handle_udp(c_clients);
    });
    let mut f = File::open("/dev/hidraw2").unwrap();
    loop {
        let c_clients = Arc::clone(&clients);
        let mut packet = [0; 77];
        let count = f.read(&mut packet).unwrap();
        assert_eq!(count, 77);
        assert_eq!(packet[0], 0x11);
        for (addr, s) in c_clients.read().unwrap().iter() {
            s.send(packet).unwrap()
        }
    }
    handle.join().unwrap();
}
