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
use signal_hook::consts::signal::*;

mod common;
mod ds4;
mod dsense;
mod udevmon;

use common::{DS4PacketInner, Packet, PACKET_LEN_USB};
use ds4::{Controls, DS4Controls, DS4PacketBT, DS4PacketUSB};
use dsense::{DSensePacketBT, DSensePacketUSB};
use udevmon::{DSType, Gamepads};

type Clients = HashMap<SocketAddr, Sender<DS4PacketUSB>>;

fn send_to_client(
    addr: SocketAddr,
    client: UdpSocket,
    hidraw_path: String,
    gamepad_type: DSType,
    global_stop: Arc<AtomicBool>,
) {
    eprintln!("Handling new client {} with gamepad {}", addr, hidraw_path);
    let mut f_read = File::open(&hidraw_path).unwrap();
    while !global_stop.load(Ordering::SeqCst) {
        let mut packet: Box<dyn Packet> = match gamepad_type {
            DSType::DS4BT => Box::new(DS4PacketBT::new()),
            DSType::DS4USB => Box::new(DS4PacketUSB::new()),
            DSType::SenseUSB => Box::new(DSensePacketUSB::new()),
            DSType::SenseBT => Box::new(DSensePacketBT::new()),
        };
        let mut new_packet: DS4PacketInner = [0; PACKET_LEN_USB];
        match packet.read(&mut f_read) {
            Ok(()) => {
                let battery_capacity = packet.battery_capacity();
                //if battery_capacity == 0 {
                //    low_bat.store(true, Ordering::SeqCst);
                //}
                new_packet = packet.to_ds4_packet();
            }
            Err(err) => {
                //eprintln!("Error while reading from gamepad src={} err={}", addr, err);
                new_packet[0] = 0x77;
                break;
            }
        }
        match client.send_to(&new_packet, addr) {
            Ok(_) => true,
            Err(err) => {
                eprintln!("Error on address src={} err={}", addr, err);
                break;
            }
        };
    }
    eprintln!("Thread stopped for {}", addr);
}

fn handle_disconnect(addr: SocketAddr, clients: &mut Clients, gamepads: &Gamepads) {
    let _ = clients.remove(&addr);
    match gamepads
        .write()
        .iter_mut()
        .find(|(k, v)| v.used_by == Some(addr))
    {
        Some((syspath, gamepad)) => gamepad.used_by = None,
        None => (),
    }
    eprintln!(
        "Client {} disconnected gracefully {:?}",
        addr,
        gamepads.read()
    );
}

fn control<T>(ctrl: T, writer: &mut impl Write) -> io::Result<()>
where
    T: Controls,
{
    let pkt = ctrl.make_packet_with_checksum();
    match writer.write(&pkt) {
        Ok(count) => assert_eq!(count, PACKET_LEN_USB),
        Err(e) => return Err(e),
    };
    writer.flush()
}

fn handle_rumble(
    large: u8,
    small: u8,
    writer: &mut impl Write,
    low_bat: &Arc<AtomicBool>,
) -> io::Result<()> {
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

fn handle_new_client(
    src: SocketAddr,
    sock_r: UdpSocket,
    sock_w: UdpSocket,
    clients: &mut Clients,
    gamepads: &Gamepads,
    global_stop: Arc<AtomicBool>,
) {
    //let mut thread_name = format!("send_to_client {}", src);
    let mut selected_gamepad_path = String::from("");
    let mut selected_key = String::from("");
    let mut gamepad_type: Option<DSType> = None;
    match gamepads
        .write()
        .iter_mut()
        .find(|(k, v)| v.used_by.is_none())
    {
        Some((syspath, gamepad)) => {
            gamepad.used_by = Some(src);
            selected_gamepad_path = gamepad.hidraw_path.to_string();
            gamepad_type = Some(gamepad.gamepad_type);
            selected_key = syspath.to_string();
        }
        None => {
            //eprintln!("No gamepads available");
            return;
        }
    }
    let (s, r) = unbounded();
    clients.insert(src, s);
    eprintln!("New client connected {:?}", clients);
    eprintln!("Gamepads after connect {:?}", gamepads);
    let thread_name = format!("send_to_client {}", src);
    //let stop_thread = Arc::clone(&global_stop);
    if let Err(err) = thread::Builder::new().name(thread_name).spawn(move || {
        send_to_client(
            src,
            sock_w,
            selected_gamepad_path,
            gamepad_type.unwrap(),
            global_stop,
        )
    }) {
        eprintln!("Error in creating thread for client {}: {}", src, err);
        //global_stop.store(true, Ordering::SeqCst);
    }
}

//fn handle_udp(clients: Clients, stop: Arc<AtomicBool>, writer: &mut impl Write, low_bat: Arc<AtomicBool>) -> io::Result<()> {
fn handle_udp(
    mut clients: Clients,
    global_stop: Arc<AtomicBool>,
    low_bat: Arc<AtomicBool>,
    gamepads: Gamepads,
) -> io::Result<()> {
    let mut buf = [0u8; 4];
    let socket = UdpSocket::bind("[::]:9999")?;
    //let mut writer = unsafe { File::from_raw_fd(1) };
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    while !global_stop.load(Ordering::SeqCst) {
        let (amt, src) = match socket.recv_from(&mut buf) {
            Ok((amt, src)) => (amt, src),
            Err(_e) => continue,
        };
        let buf = &mut buf[..amt];
        match buf[0] {
            0 => {
                let sock_r = socket.try_clone()?;
                let sock_w = socket.try_clone()?;
                let global_stop = Arc::clone(&global_stop);
                //let clients = Arc::clone(&clients);
                //let gamepads = Arc::clone(&gamepads);
                handle_new_client(src, sock_r, sock_w, &mut clients, &gamepads, global_stop);
                //let sock_r = thread_check!(socket.try_clone(), stop);
                //let mut thread_name = format!("send_to_client {}", src);
                /*let mut selected_gamepad_path = String::from("");
                let mut selected_key = String::from("");
                let mut gamepad_type: Option<udevmon::DSType> = None;
                match gamepads
                    .write()
                    .iter_mut()
                    .find(|(k, v)| v.used_by.is_none())
                {
                    Some((syspath, gamepad)) => {
                        gamepad.used_by = Some(src);
                        selected_gamepad_path = gamepad.hidraw_path.to_string();
                        gamepad_type = Some(gamepad.gamepad_type);
                        selected_key = syspath.to_string();
                    }
                    None => {*/
                //eprintln!("No gamepads available");
                /*        continue;
                    }
                }
                let (s, r) = unbounded();
                clients.write().insert(src, s);
                eprintln!("New client connected {:?}", clients.read());
                eprintln!("Gamepads after connect {:?}", gamepads);
                let thread_name = format!("send_to_client {}", src);
                let stop_thread = Arc::clone(&stop);
                let cloned_gamepads = Arc::clone(&gamepads);
                let sock_w = thread_check!(socket.try_clone(), stop);
                if let Err(err) = thread::Builder::new().name(thread_name).spawn(move || {
                    send_to_client(
                        src,
                        sock_w,
                        selected_gamepad_path,
                        gamepad_type.unwrap(),
                        stop_thread,
                    )
                }) {
                    eprintln!("Error in creating thread for client {}: {}", src, err);
                    stop.store(true, Ordering::SeqCst);
                }*/
            }
            1 => eprintln!("No rumble yet"), //handle_rumble(buf[1], buf[2], writer, &low_bat)?,
            2 => handle_disconnect(src, &mut clients, &gamepads),
            _ => panic!("Bohuzel"),
        };
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // synchronization stuff
    let gamepads: Gamepads = Arc::new(RwLock::new(HashMap::new()));
    let clients: Clients = HashMap::new(); //Arc::new(RwLock::new(HashMap::new()));
    udevmon::start_monitor(&gamepads);

    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register_conditional_shutdown(SIGTERM, 1, Arc::clone(&stop))?;
    signal_hook::flag::register_conditional_shutdown(SIGQUIT, 1, Arc::clone(&stop))?;
    signal_hook::flag::register(SIGTERM, Arc::clone(&stop))?;
    signal_hook::flag::register(SIGQUIT, Arc::clone(&stop))?;

    let low_bat = Arc::new(AtomicBool::new(false));

    //let mut f_read = unsafe { File::from_raw_fd(0) };
    //let mut f_read = File::open("/dev/hidraw0")?;
    //let mut stdin = io::stdin();
    // stdin could be used for writing, but systemd
    // do not want to open file for writing if we're not
    // using stdout or stderr
    // Could use a io::Stdin here, but it's line buffered
    //let mut f_write = unsafe { File::from_raw_fd(1) };
    //handle_rumble(0, 0, &mut f_write, &Arc::new(AtomicBool::new(false)))?;
    //let mut f_write_clone = f_write.try_clone()?;

    //let handle = {
    //    let clients: Clients = Arc::clone(&clients);
    //    let stop = Arc::clone(&stop);
    //    let low_bat = Arc::clone(&low_bat);
    //    thread::Builder::new()
    //        .name("handle_udp!".to_string())
    //        //.spawn(move || handle_udp(clients, stop, &mut f_write, low_bat))?
    //        .spawn(move || handle_udp(clients, stop, low_bat))?
    //};

    //let mut battery = 0;
    //{
    //	let clients: Clients = Arc::clone(&clients);
    //	let stop = Arc::clone(&stop);
    //	let low_bat = Arc::clone(&low_bat);
    handle_udp(clients, stop, low_bat, gamepads)?;
    //    }
    //while !stop.load(Ordering::SeqCst) {
    //let mut packet: DSensePacket = [0; DSENSE_PACKET_LEN_BT];
    //handle_udp(clients, sto
    /*match //f_read.read(&mut packet) {
        Ok(count) => {
            assert_eq!(count, DSENSE_PACKET_LEN_BT);
            assert!(packet.is_valid());
            let battery_capacity = packet.battery_capacity();
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
    let new_packet = packet.to_ds4_packet();
    for (_addr, s) in clients.read().iter() {
        let _ = s.send(new_packet);
    }*/
    //}
    //handle.join().unwrap()?;
    Ok(())
}
