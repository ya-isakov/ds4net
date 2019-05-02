use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;

use crc::crc32;
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;

const DEFAULT_LATENCY: u8 = 4;

type Packet = [u8; 77];
type Clients = Arc<RwLock<HashMap<SocketAddr, Sender<Packet>>>>;

fn send_to_client(addr: SocketAddr, r: &Receiver<Packet>, client: UdpSocket) {
    println!("addr: {}", addr);
    loop {
        let packet: Packet = r.recv().unwrap();
        if packet[0] == 0xFE && packet[1] == 0xFE {
            println!("Disconnected {}", addr);
            break;
        }
        match client.send_to(&packet, addr) {
            Ok(_) => true,
            Err(err) => {
                println!("Error on address {} {}", addr, err);
                break;
            }
        };
    }
}

fn handle_disconnect(addr: SocketAddr, clients: &Clients) {
    let disconnect_packet: Packet = [0xFE; 77];
    let sender = clients.write().remove(&addr).unwrap();
    sender.send(disconnect_packet).unwrap();
}

fn get_control(_val: &str) -> u8 {
    0
}

fn transform_u32_to_array_of_u8(x: u32) -> [u8; 4] {
    let b0: u8 = ((x >> 24) & 0xff) as u8;
    let b1: u8 = ((x >> 16) & 0xff) as u8;
    let b2: u8 = ((x >> 8) & 0xff) as u8;
    let b3: u8 = (x & 0xff) as u8;
    [b3, b2, b1, b0]
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

fn handle_udp(clients: Clients) {
    let socket = UdpSocket::bind("0.0.0.0:9999").unwrap();
    let mut buf: [u8; 4] = [0; 4];
    loop {
        let (amt, src) = socket.recv_from(&mut buf).unwrap();
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let (s, r) = unbounded();
                clients.write().insert(src, s);
                let new_socket = socket.try_clone().unwrap();
                let thread_name = format!("{}", src);
                thread::Builder::new()
                    .name(thread_name)
                    .spawn(move || send_to_client(src, &r, new_socket))
                    .unwrap();
            }
            1 => handle_rumble(),
            2 => handle_disconnect(src, &clients),
            _ => panic!("Bohuzel"),
        };
    }
}

fn main() {
    handle_rumble();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let c_clients: Clients = Arc::clone(&clients);
    thread::Builder::new()
        .name("handle_udp".to_string())
        .spawn(move || handle_udp(c_clients))
        .unwrap();
    let mut f = File::open("/dev/hidraw0").unwrap();
    //let c_clients = Arc::clone(&clients);
    loop {
        let mut packet: Packet = [0; 77];
        let count = f.read(&mut packet).unwrap();
        assert_eq!(count, 77);
        assert_eq!(packet[0], 0x11);
        for (_addr, s) in clients.read().iter() {
            let _ = s.send(packet);
        }
    }
    //handle.join().unwrap();
}
