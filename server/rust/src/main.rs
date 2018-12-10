use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;

extern crate crossbeam_channel;
use crossbeam_channel::{unbounded, Receiver, Sender};

extern crate parking_lot;
use parking_lot::RwLock;

extern crate crc;
use crc::crc32;

const DEFAULT_LATENCY: u8 = 4;

fn handle_new_client(addr: SocketAddr, r: Receiver<[u8; 77]>) {
    println!("addr: {}", addr);
    let client = UdpSocket::bind("0.0.0.0:0").unwrap();
    client.connect(addr).unwrap();
    let mut connected: bool = false;
    loop {
        let packet: [u8; 77] = r.recv().unwrap();
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

//fn handle_disconnect(addr: SocketAddr, _clients: &mut Vec<SocketAddr>) {}

fn get_control(_val: &str) -> u8 {
    0
}

fn transform_u32_to_array_of_u8(x: u32) -> [u8; 4] {
    let b0: u8 = ((x >> 24) & 0xff) as u8;
    let b1: u8 = ((x >> 16) & 0xff) as u8;
    let b2: u8 = ((x >> 8) & 0xff) as u8;
    let b3: u8 = (x & 0xff) as u8;
    return [b3, b2, b1, b0];
}

fn fill_packet() -> [u8; 78] {
    let mut pkt: [u8; 78] = [0; 78];
    pkt[0] = 0x11;
    pkt[1] = 0xC0 | DEFAULT_LATENCY;
    pkt[3] = 0x07;
    pkt[6] = get_control("small_rumble");
    pkt[7] = get_control("big_rumble");
    pkt[8] = get_control("r");
    pkt[9] = get_control("g");
    pkt[10] = get_control("b");
    // Time to flash bright (255 = 2.5 seconds)
    pkt[11] = 0; // min(flash_led1, 255)
    // Time to flash dark (255 = 2.5 seconds)
    pkt[12] = 0; // min(flash_led2, 255)
    pkt[21] = get_control("volume_l");
    pkt[22] = get_control("volume_r");
    pkt[23] = 0x49; // magic
    pkt[24] = get_control("volume_speaker");
    pkt[25] = 0x85; //magic
    pkt
}

fn checksum(packet: &[u8]) -> [u8; 4] {
    let mut full_packet: [u8; 75] = [0; 75];
    full_packet[0] = 0xA2;
    full_packet[1..].copy_from_slice(packet);
    let hasher = crc32::checksum_ieee(&full_packet);
    transform_u32_to_array_of_u8(hasher.to_le())
}

fn handle_rumble() {
    let mut pkt = fill_packet();
    let crc = checksum(&pkt[0..74]);
    pkt[74..78].copy_from_slice(&crc);
    println!("{:?}", &pkt[..]);
}

fn handle_udp(clients: Arc<RwLock<HashMap<SocketAddr, Sender<[u8; 77]>>>>) {
    let socket = UdpSocket::bind("127.0.0.1:9999").unwrap();
    let mut buf: [u8; 4] = [0; 4];
    loop {
        let (amt, src) = socket.recv_from(&mut buf).unwrap();
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let (s, r) = unbounded();
                clients.write().insert(src, s);
                let _handle = thread::spawn(move || handle_new_client(src, r));
            }
            1 => handle_rumble(),
            _ => panic!("Bohuzel"),
        };
    }
}

fn main() {
    handle_rumble();
    let clients = Arc::new(RwLock::new(HashMap::new()));
    let c_clients = Arc::clone(&clients);
    let _handle = thread::spawn(move || {
        handle_udp(c_clients);
    });
    let mut f = File::open("/dev/hidraw2").unwrap();
    loop {
        let c_clients = Arc::clone(&clients);
        let mut packet = [0; 77];
        let count = f.read(&mut packet).unwrap();
        assert_eq!(count, 77);
        assert_eq!(packet[0], 0x11);
        for (_addr, s) in c_clients.read().iter() {
            s.send(packet).unwrap()
        }
    }
    //handle.join().unwrap();
}
