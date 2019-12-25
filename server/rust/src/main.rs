use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

use crc::crc32;
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;

const DEFAULT_LATENCY: u8 = 4;

type Packet = [u8; 77];
type Clients = Arc<RwLock<HashMap<SocketAddr, Sender<Packet>>>>;

fn send_to_client(addr: SocketAddr, r: &Receiver<Packet>, client: UdpSocket) {
    println!("addr: {}", addr);
    while let Ok(packet) = r.recv() {
        match client.send_to(&packet, addr) {
            Ok(_) => true,
            Err(err) => {
                println!("Error on address {} {}", addr, err);
                break;
            }
        };
    }
    println!("Disconnected {}", addr);
}

fn handle_disconnect(addr: SocketAddr, clients: &Clients) {
    let _ = clients.write().remove(&addr);
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

fn fill_packet(large: u8, small: u8) -> [u8; 78] {
    let mut pkt = [0u8; 78];
    pkt[0] = 0x11;
    pkt[1] = 0xC0 | DEFAULT_LATENCY;
    pkt[3] = 0x07;
    pkt[6] = small;
    pkt[7] = large;
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
    let mut full_packet = [0u8; 75];
    full_packet[0] = 0xA2;
    full_packet[1..].copy_from_slice(packet);
    let hasher = crc32::checksum_ieee(&full_packet);
    transform_u32_to_array_of_u8(hasher.to_le())
}

fn handle_rumble(large: u8, small: u8, f: &mut File) -> io::Result<()> {
    println!("Got rumble {} {}", large, small);
    let mut pkt = fill_packet(large, small);
    let crc = checksum(&pkt[0..74]);
    pkt[74..78].copy_from_slice(&crc);
    f.write(&pkt)?;
    println!("{:?}", &pkt[..]);
    Ok(())
}

// Signal that udp thread is stopped because of error
macro_rules! thread_check {
    ( $x:expr, $r:expr ) => {
        match $x {
            Ok(var) => var,
            Err(err) => {
                $r.store(false, Ordering::SeqCst);
                return Err(err);
            }
        }
    };
}

fn handle_udp(clients: Clients, running: Arc<AtomicBool>, mut f: File) -> io::Result<()> {
    let mut buf = [0u8; 4];
    let socket = thread_check!(UdpSocket::bind("0.0.0.0:9999"), running);
    while running.load(Ordering::SeqCst) {
        let (amt, src) = thread_check!(socket.recv_from(&mut buf), running);
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let (s, r) = unbounded();
                clients.write().insert(src, s);
                let new_socket = thread_check!(socket.try_clone(), running);
                let thread_name = format!("{}", src);
                if let Err(err) = thread::Builder::new()
                    .name(thread_name)
                    .spawn(move || send_to_client(src, &r, new_socket))
                {
                    running.store(false, Ordering::SeqCst);
                    panic!("{}", err);
                }
            }
            1 => handle_rumble(buf[1], buf[2], &mut f)?,
            2 => handle_disconnect(src, &clients),
            _ => panic!("Bohuzel"),
        };
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // synchronization stuff
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let c_clients: Clients = Arc::clone(&clients);
    let udp_running = Arc::new(AtomicBool::new(true));
    let r = udp_running.clone();

    let mut f = OpenOptions::new().read(true).write(true).open("/dev/hidraw0")?;
    handle_rumble(0, 0, &mut f)?;
    let f_cloned = f.try_clone()?;

    let handle = thread::Builder::new()
        .name("handle_udp!".to_string())
        .spawn(move || handle_udp(c_clients, r, f_cloned))?;

    while udp_running.load(Ordering::SeqCst) {
        let mut packet: Packet = [0; 77];
        match f.read(&mut packet) {
            Ok(count) => {
                assert_eq!(count, 77);
                assert_eq!(packet[0], 0x11);
            },
            Err(e) => {
                udp_running.store(false, Ordering::SeqCst);
                println!("Error while reading from device: {}", e);
                break;
            }
        }
        for (_addr, s) in clients.read().iter() {
            let _ = s.send(packet);
        }
    }

    handle.join().unwrap()?;
    handle_rumble(0, 0, &mut f)?;
    Ok(())
}
