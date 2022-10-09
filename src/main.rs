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

use crossbeam_channel::{unbounded, Receiver, Sender};
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

fn send_to_client(
    addr: SocketAddr,
    client: UdpSocket,
    mut f_read: File,
    gamepad_type: DSType,
    global_stop: Arc<AtomicBool>,
    battery_sender: Sender<ControlType>,
) {
    let mut battery_level = 0;
    //let mut f_read = File::open(&hidraw_path).unwrap();
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
                if battery_capacity != battery_level {
                    eprintln!(
                        "Battery level changed for {} to {}%",
                        addr, battery_capacity
                    );
                    battery_sender.send(ControlType::Battery(battery_capacity)).unwrap();
                    battery_level = battery_capacity;
                }
                new_packet = packet.to_ds4_packet();
            }
            Err(_err) => {
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
        .find(|(_k, v)| v.used_by == Some(addr))
    {
        Some((_syspath, gamepad)) => gamepad.used_by = None,
        None => (),
    }
    eprintln!(
        "Client {} disconnected gracefully {:?}",
        addr,
        gamepads.read()
    );
}

pub enum ControlType {
    Rumble { large: u8, small: u8 },
    Color { r: u8, g: u8, b: u8 },
    Battery(u8),
}

fn control_ds4(
    mut f_write: File,
    r: Receiver<ControlType>,
    global_stop: Arc<AtomicBool>,
    is_bt: bool,
) {
    let mut ds4c: DS4Controls = Default::default();
    while !global_stop.load(Ordering::SeqCst) {
        match r.recv().unwrap() {
            ControlType::Rumble { large, small } => {
                ds4c.large = large;
                ds4c.small = small;
            }
            ControlType::Color { r, g, b } => {
                ds4c.red = r;
                ds4c.green = g;
                ds4c.blue = b;
            }
            ControlType::Battery(level) => {
                ds4c.battery = level;
            }
        }
        if is_bt {
            ds4c.write_packet_bt(&mut f_write).unwrap()
        } else {
            ds4c.write_packet_usb(&mut f_write).unwrap()
        }
    }
}

fn control_sense_usb(
    mut f_write: File,
    r: Receiver<ControlType>,
    global_stop: Arc<AtomicBool>,
    is_bt: bool,
) {
    let mut sense: DSenseControls = Default::default();
    while !global_stop.load(Ordering::SeqCst) {
        match r.recv().unwrap() {
            ControlType::Rumble { large, small } => {
                sense.large = large;
                sense.small = small;
            }
            ControlType::Color { r, g, b } => {
                sense.red = r;
                sense.green = g;
                sense.blue = b;
            }
            ControlType::Battery(level) => {
                sense.battery = level;
            }
        }
        if is_bt {
            sense.write_packet_bt(&mut f_write).unwrap()
        } else {
            sense.write_packet_usb(&mut f_write).unwrap()
        }
    }
}

fn control_gamepad(
    gamepad_type: DSType,
    f_write: File,
    r: Receiver<ControlType>,
    global_stop: Arc<AtomicBool>,
) {
    match gamepad_type {
        DSType::DS4USB => control_ds4(f_write, r, global_stop, false),
        DSType::DS4BT => control_ds4(f_write, r, global_stop, true),
        DSType::SenseUSB => control_sense_usb(f_write, r, global_stop, false),
        DSType::SenseBT => control_sense_usb(f_write, r, global_stop, true),
    }
}

fn handle_new_client(
    src: SocketAddr,
    sock_w: UdpSocket,
    clients: &mut Clients,
    gamepads: &Gamepads,
    global_stop: Arc<AtomicBool>,
) {
    let (gamepad_type, gamepad_hidraw, f_read, f_write) = match gamepads
        .write()
        .iter_mut()
        .find(|(_k, v)| v.used_by.is_none())
    {
        Some((_syspath, gamepad)) => {
            gamepad.used_by = Some(src);
            let f_write = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&gamepad.hidraw_path)
                .unwrap();
            let f_read = f_write.try_clone().unwrap();
            //syspath.to_string(),
            (
                gamepad.gamepad_type,
                gamepad.hidraw_path.to_string(),
                f_read,
                f_write,
            )
        }
        None => {
            //eprintln!("No gamepads available");
            return;
        }
    };
    let (s, r) = unbounded();
    clients.insert(src, s.clone());
    eprintln!("New client connected {:?}", clients);
    eprintln!("Gamepads after connect {:?}", gamepads);
    let send_thread_name = format!("send_to_client {}", src);
    let control_thread_name = format!("handle_control {}", gamepad_hidraw);
    eprintln!(
        "Starting thread for handling new client {} with gamepad {}",
        src, gamepad_hidraw
    );
    let stop_thread = Arc::clone(&global_stop);
    let battery_sender = s.clone();
    if let Err(err) = thread::Builder::new()
        .name(send_thread_name)
        .spawn(move || {
            send_to_client(
                src,
                sock_w,
                f_read,
                gamepad_type,
                stop_thread,
                battery_sender,
            )
        })
    {
        eprintln!("Error in creating thread for client {}: {}", src, err);
        //global_stop.store(true, Ordering::SeqCst);
        return;
    }
    let stop_thread = Arc::clone(&global_stop);
    if let Err(err) = thread::Builder::new()
        .name(control_thread_name)
        .spawn(move || control_gamepad(gamepad_type, f_write, r, stop_thread))
    {
        eprintln!(
            "Error in creating control thread for client {}: {}",
            src, err
        );
        return;
    }
    s.send(ControlType::Rumble {
        large: 0,
        small: 255,
    })
    .unwrap();
    thread::sleep(Duration::from_secs(1));
    s.send(ControlType::Color { r: 0, g: 0, b: 255 }).unwrap();
    s.send(ControlType::Rumble { large: 0, small: 0 }).unwrap();
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
            0 => {
                let sock_w = socket.try_clone()?;
                let global_stop = Arc::clone(&global_stop);
                handle_new_client(src, sock_w, &mut clients, &gamepads, global_stop);
            }
            1 => clients[&src]
                .send(ControlType::Rumble {
                    large: buf[1],
                    small: buf[2],
                })
                .unwrap(),
            2 => handle_disconnect(src, &mut clients, &gamepads),
            _ => panic!("Bohuzel"),
        };
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let gamepads: Gamepads = Arc::new(RwLock::new(HashMap::new()));
    let clients: Clients = HashMap::new();
    udevmon::start_monitor(&gamepads);

    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register_conditional_shutdown(SIGTERM, 1, Arc::clone(&stop))?;
    signal_hook::flag::register_conditional_shutdown(SIGQUIT, 1, Arc::clone(&stop))?;
    signal_hook::flag::register(SIGTERM, Arc::clone(&stop))?;
    signal_hook::flag::register(SIGQUIT, Arc::clone(&stop))?;

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
