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

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;

mod ds4;

use ds4::DS4Controls;

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

fn handle_rumble(large: u8, small: u8, f: &mut File) -> io::Result<()> {
    println!("Got rumble {} {}", large, small);
    let mut ds4c: DS4Controls = Default::default();
    ds4c.large = large;
    ds4c.small = small;
    println!("DS4Controls {:?}", ds4c);
    let pkt = ds4c.make_packet_with_checksum();
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

    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/hidraw0")?;
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
            }
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
