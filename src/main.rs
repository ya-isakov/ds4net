use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use parking_lot::RwLock;
use signal_hook::consts::signal::*;

mod common_input;
mod common_output;
mod controls_ds4;
mod controls_dsense;
mod input_ds4;
mod input_dsense;
mod udevmon;

use common_input::{DS4PacketInner, Packet, PACKET_LEN_USB};
use common_output::Controls;
use controls_ds4::DS4Controls;
use controls_dsense::DSenseControls;
use input_ds4::{DS4PacketBT, DS4PacketUSB};
use input_dsense::{DSensePacketBT, DSensePacketUSB};
use udevmon::{DSType, Gamepads};

type Clients = HashMap<SocketAddr, Sender<ControlType>>;
type SendFunc =
    fn(SocketAddr, UdpSocket, File, Arc<AtomicBool>, Arc<AtomicBool>, Sender<ControlType>);
type ControlFunc = fn(File, Receiver<ControlType>, Arc<AtomicBool>, Arc<AtomicBool>, bool);

pub enum ControlType {
    Rumble { large: u8, small: u8 },
    Color { r: u8, g: u8, b: u8 },
    Battery(u8),
}

fn find_and_open_gamepad(
    gamepads: &Gamepads,
    src: SocketAddr,
) -> Option<(DSType, String, File, File)> {
    let mut locked_gamepads = gamepads.write();
    let (_syspath, gamepad) = locked_gamepads
        .iter_mut()
        .find(|(_k, v)| v.used_by.is_none())?;
    gamepad.used_by = Some(src);
    let f_write = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&gamepad.hidraw_path)
        .unwrap();
    let f_read = f_write.try_clone().unwrap();
    //syspath.to_string(),
    Some((
        gamepad.gamepad_type,
        gamepad.hidraw_path.to_string(),
        f_read,
        f_write,
    ))
}

fn send_to_client<T: Packet + Default>(
    addr: SocketAddr,
    client: UdpSocket,
    mut f_read: File,
    global_stop: Arc<AtomicBool>,
    client_stop: Arc<AtomicBool>,
    sender: Sender<ControlType>,
) {
    let mut bat_level = 0;
    //let mut f_read = File::open(&hidraw_path).unwrap();
    while !client_stop.load(Ordering::SeqCst) && !global_stop.load(Ordering::SeqCst) {
        let mut packet: T = Default::default();
        let mut new_packet: DS4PacketInner = [0; PACKET_LEN_USB];
        match packet.read(&mut f_read) {
            Ok(()) => {
                let capacity = packet.battery_capacity();
                if capacity != bat_level {
                    eprintln!("Battery level changed for {} to {}%", addr, capacity);
                    sender.send(ControlType::Battery(capacity)).unwrap();
                    bat_level = capacity;
                }
                new_packet = packet.to_ds4_packet();
            }
            Err(_err) => {
                //eprintln!("Error while reading from gamepad src={} err={}", addr, err);
                new_packet[0] = 0x77;
                break;
            }
        }
        if let Err(err) = client.send_to(&new_packet, addr) {
            eprintln!("Error on address src={} err={}", addr, err);
            break;
        };
    }
    eprintln!("Input thread stopped for {}", addr);
    client_stop.store(true, Ordering::SeqCst);
}

fn create_input_thread(
    src: SocketAddr,
    ds_type: DSType,
    global_stop: &Arc<AtomicBool>,
    client_stop: &Arc<AtomicBool>,
    socket: &UdpSocket,
    f_read: File,
    s: &Sender<ControlType>,
) -> Option<()> {
    let send_thread_name = format!("send_to_client_{}", src);
    let global_stop = Arc::clone(global_stop);
    let client_stop = Arc::clone(client_stop);
    let sender = s.clone();
    let sock_w = socket.try_clone().unwrap();
    let f: SendFunc = match ds_type {
        DSType::DS4BT => send_to_client::<DS4PacketBT>,
        DSType::DS4USB => send_to_client::<DS4PacketUSB>,
        DSType::SenseBT => send_to_client::<DSensePacketBT>,
        DSType::SenseUSB => send_to_client::<DSensePacketUSB>,
    };
    if let Err(err) = thread::Builder::new()
        .name(send_thread_name)
        .spawn(move || f(src, sock_w, f_read, global_stop, client_stop, sender))
    {
        eprintln!("Error creating input thread for client {}: {}", src, err);
        return None;
    }
    s.send(ControlType::Color { r: 0, g: 255, b: 0 }).unwrap();
    thread::sleep(Duration::from_secs(1));
    s.send(ControlType::Color { r: 0, g: 0, b: 255 }).unwrap();
    Some(())
}

fn control_dsc<T: Controls + Default>(
    mut f_write: File,
    r: Receiver<ControlType>,
    global_stop: Arc<AtomicBool>,
    client_stop: Arc<AtomicBool>,
    is_bt: bool,
) {
    let mut dsc: T = Default::default();
    while !global_stop.load(Ordering::SeqCst) && !client_stop.load(Ordering::SeqCst) {
        match r.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => {
                match result {
                    ControlType::Rumble { large, small } => {
                        dsc.set_rumble(large, small);
                    }
                    ControlType::Color { r, g, b } => {
                        dsc.set_color(r, g, b);
                    }
                    ControlType::Battery(level) => {
                        dsc.set_battery(level);
                    }
                }
                if is_bt {
                    dsc.write_packet_bt(&mut f_write).unwrap()
                } else {
                    dsc.write_packet_usb(&mut f_write).unwrap()
                }
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(RecvTimeoutError::Disconnected) => {
                client_stop.store(true, Ordering::SeqCst);
                break;
            }
        }
    }
    eprintln!("Control thread stopped");
}

fn create_control_thread(
    src: SocketAddr,
    ds_type: DSType,
    global_stop: &Arc<AtomicBool>,
    client_stop: &Arc<AtomicBool>,
    f_write: File,
    r: Receiver<ControlType>,
) -> Option<()> {
    let control_thread_name = format!("handle_control_{}", src);
    let global_stop = Arc::clone(global_stop);
    let client_stop_thread = Arc::clone(client_stop);
    let x: (ControlFunc, bool) = match ds_type {
        DSType::DS4USB => (control_dsc::<DS4Controls>, false),
        DSType::DS4BT => (control_dsc::<DS4Controls>, true),
        DSType::SenseUSB => (control_dsc::<DSenseControls>, false),
        DSType::SenseBT => (control_dsc::<DSenseControls>, true),
    };
    // NOTE: until https://github.com/rust-lang/rfcs/issues/2870 is fixed and in stable
    let (f, is_bt) = x;
    if let Err(err) = thread::Builder::new()
        .name(control_thread_name)
        .spawn(move || f(f_write, r, global_stop, client_stop_thread, is_bt))
    {
        eprintln!("Error creating control thread for client {}: {}", src, err);
        client_stop.store(true, Ordering::SeqCst);
        return None;
    }
    Some(())
}

fn handle_new_client(
    src: SocketAddr,
    socket: &UdpSocket,
    clients: &mut Clients,
    gamepads: &Gamepads,
    global_stop: &Arc<AtomicBool>,
) {
    let (ds_type, hidraw, f_read, f_write) = match find_and_open_gamepad(gamepads, src) {
        Some((ds_type, hidraw, f_read, f_write)) => (ds_type, hidraw, f_read, f_write),
        None => return,
    };
    eprintln!("New client connected {:?}", clients);
    eprintln!("Gamepads after connect {:?}", gamepads);
    let client_stop = Arc::new(AtomicBool::new(false));
    let (s, r) = unbounded();
    eprintln!("Starting control thread for {} {}", src, hidraw);
    if create_control_thread(src, ds_type, global_stop, &client_stop, f_write, r).is_none() {
        return;
    };
    eprintln!("Starting input thread for {}, gamepad {}", src, hidraw);
    if create_input_thread(src, ds_type, global_stop, &client_stop, socket, f_read, &s).is_none() {
        return;
    }
    clients.insert(src, s);
}

fn handle_rumble(clients: &Clients, src: SocketAddr, large: u8, small: u8) {
    if let Some(sender) = clients.get(&src) {
        sender.send(ControlType::Rumble { large, small }).unwrap();
    };
}

fn handle_disconnect(addr: SocketAddr, clients: &mut Clients, gamepads: &Gamepads) {
    clients.remove(&addr);
    if let Some((_syspath, gamepad)) = gamepads
        .write()
        .iter_mut()
        .find(|(_k, v)| v.used_by == Some(addr))
    {
        gamepad.used_by = None;
    }
    eprintln!("Client {} disconnected {:?}", addr, gamepads.read());
}

fn handle_udp(
    mut clients: Clients,
    global_stop: Arc<AtomicBool>,
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
            0 => handle_new_client(src, &socket, &mut clients, &gamepads, &global_stop),
            1 => handle_rumble(&clients, src, buf[1], buf[2]),
            2 => handle_disconnect(src, &mut clients, &gamepads),
            _ => panic!("Bohuzel"),
        };
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let gamepads: Gamepads = Arc::new(RwLock::new(HashMap::new()));
    let clients: Clients = HashMap::new();
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register_conditional_shutdown(SIGTERM, 1, Arc::clone(&stop))?;
    signal_hook::flag::register_conditional_shutdown(SIGQUIT, 1, Arc::clone(&stop))?;
    signal_hook::flag::register(SIGTERM, Arc::clone(&stop))?;
    signal_hook::flag::register(SIGQUIT, Arc::clone(&stop))?;

    udevmon::start_monitor(&gamepads, Arc::clone(&stop));

    //let mut f_read = unsafe { File::from_raw_fd(0) };
    //let mut f_read = File::open("/dev/hidraw0")?;
    //let mut stdin = io::stdin();
    // stdin could be used for writing, but systemd
    // do not want to open file for writing if we're not
    // using stdout or stderr
    // Could use a io::Stdin here, but it's line buffered
    //let mut f_write = unsafe { File::from_raw_fd(1) };
    handle_udp(clients, stop, gamepads)?;
    Ok(())
}
