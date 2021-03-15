use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::os::unix::io::FromRawFd;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;

mod ds4;

use ds4::DS4Controls;

type Packet = [u8; 78];
type Clients = Arc<RwLock<HashMap<SocketAddr, Sender<Packet>>>>;

fn send_to_client(addr: SocketAddr, r: &Receiver<Packet>, client: UdpSocket) {
    eprintln!("addr: {}", addr);
    while let Ok(packet) = r.recv() {
        // TODO: change it to be 78 bytes, when client is ready
        match client.send_to(&packet[0..77], addr) {
            Ok(_) => true,
            Err(err) => {
                eprintln!("Error on address {} {}", addr, err);
                break;
            }
        };
    }
    eprintln!("Disconnected {}", addr);
}

fn handle_disconnect(addr: SocketAddr, clients: &Clients) {
    let _ = clients.write().remove(&addr);
}

fn control(ds4c: DS4Controls, writer: &mut impl Write) -> io::Result<()> {
    eprintln!("{:p}", writer);
    let pkt = ds4c.make_packet_with_checksum();
    match writer.write(&pkt) {
        Ok(count) => assert_eq!(count, 78),
        Err(e) => return Err(e),
    };
    writer.flush()
}

fn handle_rumble(large: u8, small: u8, writer: &mut impl Write, low_bat: &Arc<AtomicBool>) -> io::Result<()> {
    eprintln!("Got rumble {} {}", large, small);
    let mut ds4c: DS4Controls = Default::default();
    if low_bat.load(Ordering::SeqCst) {
        ds4c.red = 255;
        ds4c.green = 0;
        ds4c.blue = 0;
    }
    ds4c.large = large;
    ds4c.small = small;
    control(ds4c, writer)
}

// Signal that udp thread is stopped because of error
macro_rules! thread_check {
    ( $x:expr, $r:expr ) => {
        match $x {
            Ok(var) => var,
            Err(err) => {
                $r.store(true, Ordering::SeqCst);
                return Err(err);
            }
        }
    };
}

fn handle_udp(clients: Clients, stop: Arc<AtomicBool>, writer: &mut impl Write, low_bat: Arc<AtomicBool>) -> io::Result<()> {
    eprintln!("{:p}", writer);
    let mut buf = [0u8; 4];
    let socket = thread_check!(UdpSocket::bind("0.0.0.0:9999"), stop);
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    while !stop.load(Ordering::SeqCst) {
        let (amt, src) = match socket.recv_from(&mut buf) {
            Ok((amt, src)) => (amt, src),
            Err(_e) => continue,
        };
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let (s, r) = unbounded();
                clients.write().insert(src, s);
                let new_socket = thread_check!(socket.try_clone(), stop);
                let thread_name = format!("{}", src);
                if let Err(err) = thread::Builder::new()
                    .name(thread_name)
                    .spawn(move || send_to_client(src, &r, new_socket))
                {
                    eprintln!("Error in creating thread for client {}: {}", src, err);
                    stop.store(true, Ordering::SeqCst);
                }
            }
            1 => handle_rumble(buf[1], buf[2], writer, &low_bat)?,
            2 => handle_disconnect(src, &clients),
            _ => panic!("Bohuzel"),
        };
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // synchronization stuff
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));

    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop))?;
    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop))?;

    let low_bat = Arc::new(AtomicBool::new(false));

    let mut f_read = unsafe { File::from_raw_fd(0) };
    //let mut stdin = io::stdin();
    // stdin could be used for writing, but systemd
    // do not want to open file for writing if we're not
    // using stdout or stderr
    // Could use a io::Stdin here, but it's line buffered
    let mut f_write = unsafe { File::from_raw_fd(1) };
    handle_rumble(0, 0, &mut f_write, &Arc::new(AtomicBool::new(false)))?;
    //let mut f_write_clone = f_write.try_clone()?;

    let handle = {
        let clients: Clients = Arc::clone(&clients);
        let stop = Arc::clone(&stop);
        let low_bat = Arc::clone(&low_bat);
        thread::Builder::new()
            .name("handle_udp!".to_string())
            .spawn(move || handle_udp(clients, stop, &mut f_write, low_bat))?
    };

    //let mut battery = 0;

    while !stop.load(Ordering::SeqCst) {
        let mut packet: Packet = [0; 78];
        match f_read.read(&mut packet) {
            Ok(count) => {
                assert_eq!(count, 78);
                assert_eq!(packet[0], 0x11);
                let battery_capacity = packet[32] & 0xF;
                if battery_capacity == 0 {
                    low_bat.store(true, Ordering::SeqCst);
                }
            }
            Err(e) => {
                stop.store(true, Ordering::SeqCst);
                packet[0] = 0x77;
                eprintln!("Error while reading from device: {}", e);
            }
        }
        for (_addr, s) in clients.read().iter() {
            let _ = s.send(packet);
        }
    }
    handle.join().unwrap()?;
    Ok(())
}
