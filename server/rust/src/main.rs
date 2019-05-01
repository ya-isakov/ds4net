use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::os::unix::io::FromRawFd;
use std::net::{SocketAddr, Ipv4Addr, IpAddr};
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;
use std::io;
use std::mem;

use libc;
use crc::crc32;
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;

const DEFAULT_LATENCY: u8 = 4;

const SO_ATTACH_REUSEPORT_CBPF: libc::c_int = 51;

type Packet = [u8; 77];
type Clients = Arc<RwLock<HashMap<SocketAddr, Sender<Packet>>>>;

fn send_to_client(addr: SocketAddr, r: &Receiver<Packet>, sock: UdpSocket) {
    println!("addr: {}", addr);
    //let client = UdpSocket::bind("0.0.0.0:9999").unwrap();
    //client.connect(addr).unwrap();
    let mut connected: bool = false;
    loop {
        let packet: Packet = r.recv().unwrap();
        match sock.send(&packet) {
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

fn get_sockaddr(sa: &SocketAddr) -> (*const libc::sockaddr, libc::socklen_t) {
    match *sa {
            SocketAddr::V4(ref a) => {
                (a as *const _ as *const _, mem::size_of_val(a) as libc::socklen_t)
            }
            SocketAddr::V6(ref a) => {
                (a as *const _ as *const _, mem::size_of_val(a) as libc::socklen_t)
            }
    }
}

pub type __u8 = ::std::os::raw::c_uchar;
pub type __u16 = ::std::os::raw::c_ushort;
pub type __u32 = ::std::os::raw::c_uint;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct sock_filter {
    pub code: __u16,
    pub jt: __u8,
    pub jf: __u8,
    pub k: __u32,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct sock_fprog {
    pub len: ::std::os::raw::c_ushort,
    pub filter: *mut sock_filter,
}


fn create_socket(addr: &SocketAddr, first: bool) -> UdpSocket {
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
        let optval: libc::c_int = 1;
        let ret = libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEPORT, &optval as *const _ as *const libc::c_void, mem::size_of_val(&optval) as libc::socklen_t);
        if ret != 0 {
            panic!(io::Error::last_os_error());
        }
        if first {
            //let prog = bpfprog!(1, 6 0 0 0);
            let mut ops = Vec::with_capacity(1);
            ops.push(sock_filter{code: 6, jt: 0, jf: 0, k: 0});
            let prog = sock_fprog{len: 1, filter: ops.as_mut_ptr()};
            println!("{:?}", *(prog.filter));
            let ret = libc::setsockopt(fd, libc::SOL_SOCKET, SO_ATTACH_REUSEPORT_CBPF, &prog as *const _ as *const libc::c_void, mem::size_of_val(&prog) as libc::socklen_t);
            if ret != 0 {
                panic!(io::Error::last_os_error());
            }
        }
        let (addrp, len) = get_sockaddr(addr);
        //let (addrp, len) = addr.into_inner();
        libc::bind(fd, addrp, len as _);
        UdpSocket::from_raw_fd(fd)
    }
}

fn handle_udp(clients: Clients) {
    //let socket = UdpSocket::bind("0.0.0.0:9999").unwrap();
    let socket = create_socket(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9999), false);
    let mut buf: [u8; 4] = [0; 4];
    loop {
        let (amt, src) = socket.recv_from(&mut buf).unwrap();
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let (s, r) = unbounded();
                clients.write().insert(src, s);
                let client_socket = create_socket(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9999), false);
                client_socket.connect(src).unwrap();
                thread::spawn(move || send_to_client(src, &r, client_socket));
            }
            1 => handle_rumble(),
            2 => println!("got packet"),
            _ => panic!("Bohuzel"),
        };
    }
}

fn main() {
    handle_rumble();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let c_clients: Clients = Arc::clone(&clients);
    let _handle = thread::spawn(move || {
        handle_udp(c_clients);
    });
    let mut f = File::open("/dev/hidraw0").unwrap();
    //let c_clients = Arc::clone(&clients);
    loop {
        let mut packet: Packet = [0; 77];
        let count = f.read(&mut packet).unwrap();
        assert_eq!(count, 77);
        assert_eq!(packet[0], 0x11);
        //println!("{:?}", clients);
        let mut gone_clients: Vec<SocketAddr> = Vec::new();
        for (addr, s) in clients.read().iter() {
            match s.send(packet) {
                Ok(()) => (),
                Err(_) => {
                    gone_clients.push(*addr);
                }
            };
        };
        for client in gone_clients {
            clients.write().remove(&client).unwrap();
        }
    }
    //handle.join().unwrap();
}
