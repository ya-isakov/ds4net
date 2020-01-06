use std::io;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ctrlc;

use vigemclient_sys;
use vigemclient_sys::{TargetType, ViGEm, XUsbReport, DS4_BUTTONS, XUSB_BUTTON};

type Packet = [u8; 77];

const DPADS: [u16; 9] = [0x1, 0x9, 0x8, 0xA, 0x2, 0x6, 0x4, 0x5, 0];
const DEADZONE: u8 = 7;
const HIGH_DZ: u8 = (255 / 2) + DEADZONE;
const LOW_DZ: u8 = (255 / 2) - DEADZONE;

/*fn scale_axis(v: i32, min: i32, max: i32, dz: i32) -> i32 {
    let t1 = (max + min) / 2;
    let c0 = t1 - dz;
    let c1 = t1 + dz;
    let t2 = (max - min - 4 * dz) / 2;
    let c2 = (1 << 29) / t2;
    let r0 = (c2 * (v - c0)) >> 14;
    let r1 = (c2 * (v - c1)) >> 14;
    if v < c0 {
        return clamp(r0, -32767, 32767);
    } else if v > c1 {
        return clamp(r1, -32767, 32767);
    }
    0
}*/

/*fn scale_axis(v: f32, neg: bool) -> i32 {
    //let t1 = (max + min) / 2;
    let mut temp = v / 255.;
    assert!(temp <= 1.0);
    if neg {
        //temp = (temp - 0.5) * -1.0 + 0.5;
        temp = 1. - temp;
    }
    //if (127.5 - v1).abs() > f32::from(dz) {
    (temp * 65535. - 32768.) as i32
    //}
    //0
}*/

fn scale_axis(v: u8, neg: bool) -> i32 {
    let temp = i32::from(v) * 65535 / 255;
    if neg {
        return 32767 - temp;
    }
    temp - 32768
}

fn inside_deadzone(x: u8, y: u8) -> bool {
    if ((x < HIGH_DZ) && (x > LOW_DZ)) && ((y < HIGH_DZ) && (y > LOW_DZ)) {
        return true;
    }
    false
}

fn map_buttons(ds: u16) -> u16 {
    let mut buttons = 0;
    if (ds & DS4_BUTTONS::DS4_BUTTON_CROSS as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_A as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_SQUARE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_X as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_CIRCLE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_B as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_TRIANGLE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_Y as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_THUMB_LEFT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_LEFT_THUMB as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_THUMB_RIGHT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_RIGHT_THUMB as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_SHOULDER_LEFT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_LEFT_SHOULDER as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_SHOULDER_RIGHT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_RIGHT_SHOULDER as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_SHARE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_BACK as u16;
    }
    if (ds & DS4_BUTTONS::DS4_BUTTON_OPTIONS as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_START as u16;
    }
    buttons
}

fn ds4_to_x360_packet_map(packet: &[u8]) -> XUsbReport {
    //let axis = packet[1..5].iter().map(|x| scale_axis(*x as i32, 0, 255, 5)).collect::<Vec<i32>>();
    let mut axis_lx = 0;
    let mut axis_ly = 0;
    let mut axis_rx = 0;
    let mut axis_ry = 0;
    let p1 = packet[1];
    let p2 = packet[2];
    let p3 = packet[3];
    let p4 = packet[4];
    if !inside_deadzone(p1, p2) {
        axis_lx = scale_axis(p1, false);
        axis_ly = scale_axis(p2, true);
    }
    if !inside_deadzone(p3, p4) {
        axis_rx = scale_axis(p3, false);
        axis_ry = scale_axis(p4, true);
    }
    let hat_index = (packet[5] & 0xF) as usize;
    let mut buttons = DPADS[hat_index];
    let mut ds_buttons: [u8; 2] = [0; 2];
    ds_buttons.copy_from_slice(&packet[5..7]);
    buttons |= map_buttons(u16::from_le_bytes(ds_buttons));
    XUsbReport {
        wButtons: buttons,
        bLeftTrigger: packet[8],
        bRightTrigger: packet[9],
        sThumbLX: axis_lx as i16,
        sThumbLY: axis_ly as i16,
        sThumbRX: axis_rx as i16,
        sThumbRY: axis_ry as i16,
    }
}

#[cfg(not(target_os = "windows"))]
fn handle_packet(packet: &[u8], vigem: &Option<ViGEm>) {
    if vigem.is_none() {
        let report = ds4_to_x360_packet_map(packet);
        println!("report: {:?}", report);
    }
}

#[cfg(target_os = "windows")]
fn handle_packet(packet: &[u8], vigem: &mut Option<ViGEm>) {
    if let Some(vigem) = vigem {
        let report = ds4_to_x360_packet_map(packet);
        vigem.target_x360_update(report);
    }
}

#[cfg(not(target_os = "windows"))]
fn init_vigem() -> Option<ViGEm> {
    None
}

#[cfg(target_os = "windows")]
fn init_vigem() -> Option<ViGEm> {
    let mut vigem = ViGEm::new().unwrap();
    vigem.add_target(TargetType::X360);
    Some(vigem)
}

unsafe extern "C" fn handle_notification(
        _client: vigemclient_sys::PVIGEM_CLIENT,
        _target: vigemclient_sys::PVIGEM_TARGET,
        large_motor: vigemclient_sys::UCHAR,
        small_motor: vigemclient_sys::UCHAR,
        _led_number: vigemclient_sys::UCHAR,
        user_data: vigemclient_sys::PVOID,
    ) {
    let socket = &mut *(user_data as *mut UdpSocket);
    let command_buf: [u8; 4] = [1, large_motor, small_motor, 0];
    socket.send(&command_buf).unwrap();
    println!("Got motor {}, {}, {:?}", large_motor, small_motor, socket.read_timeout());
}


fn main() -> io::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let socket = UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to address");
    socket.set_read_timeout(Some(Duration::from_millis(8)))?;
    socket
        .connect("192.168.1.2:9999")
        .expect("connect function failed");
    println!("local addr {:?}", socket.local_addr());
    let mut command_buf: [u8; 4] = [0; 4];
    socket.send(&command_buf)?;

    let mut vigem = {
        let mut socket = socket.try_clone()?;
        match init_vigem() {
            Some(v) => {
                v.register_notification(Some(handle_notification), &mut socket as *mut _ as vigemclient_sys::PVOID);
                Some(v)
            },
            None => None
        }
    };

    let mut buf: Packet = [0; 77];
    while running.load(Ordering::SeqCst) {
        match socket.recv(&mut buf) {
            Ok(_received) => {
                if buf[0] != 0x77 {
                    handle_packet(&buf[2..], &mut vigem);
                };
            },
            Err(_e) => (), //println!("recv function failed: {:?}", e),
        }
    }
    println!("Stopped");
    command_buf[0] = 2;
    socket.send(&command_buf)?;
    Ok(())
}
