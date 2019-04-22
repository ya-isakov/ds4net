use std::net::UdpSocket;

mod vigem_api_gen;
use vigem_api_gen::{DS4_BUTTONS, XUSB_BUTTON};

type Packet = [u8; 77];

const DPADS: [u16; 9] = [0x1, 0x9, 0x8, 0xA, 0x2, 0x6, 0x4, 0x5, 0];

// Drop when Ord::clamp will be stable https://github.com/rust-lang/rust/issues/44095
fn clamp(v: i32, min: i32, max: i32) -> i32 {
    assert!(min <= max);
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

fn correct_axis(v: i32, min: i32, max: i32, dz: i32) -> i32 {
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
}

fn handle_packet(received: usize, packet: &[u8]) {
    //let axis = packet[1..5].iter().map(|x| correct_axis(*x as i32, 0, 255, 5)).collect::<Vec<i32>>();
    let axis_lx = correct_axis(i32::from(packet[1]), 0, 255, 5);
    let axis_ly = correct_axis(i32::from(packet[2]), 0, 255, 5);
    let axis_rx = correct_axis(i32::from(packet[3]), 0, 255, 5);
    let axis_ry = correct_axis(i32::from(packet[4]), 0, 255, 5);
    let axis = [axis_lx, axis_ly, axis_rx, axis_ry];
    let hat_index = (packet[5] & 0xF) as usize;
    let mut buttons = DPADS[hat_index];
    let mut ds_buttons: [u8; 2] = [0; 2];
    ds_buttons.copy_from_slice(&packet[5..7]);
    let ds1 = u16::from_le_bytes(ds_buttons);
    //let ds2 = ds1 & vigem_api_gen::DS4_BUTTONS::DS4_BUTTON_CROSS as u16;
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_CROSS as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_A as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_SQUARE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_X as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_CIRCLE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_B as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_TRIANGLE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_Y as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_THUMB_LEFT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_LEFT_THUMB as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_THUMB_RIGHT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_RIGHT_THUMB as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_SHOULDER_LEFT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_LEFT_SHOULDER as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_SHOULDER_RIGHT as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_RIGHT_SHOULDER as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_SHARE as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_BACK as u16;
    }
    if (ds1 & DS4_BUTTONS::DS4_BUTTON_OPTIONS as u16) != 0 {
        buttons |= XUSB_BUTTON::XUSB_GAMEPAD_START as u16;
    }
    println!(
        "received {} bytes {} {} {:?} {} {}",
        received, packet[8], packet[9], axis, buttons, ds1
    );
}

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to address");
    let connect_packet: [u8; 4] = [0; 4];
    socket
        .connect("192.168.1.2:9999")
        .expect("connect function failed");
    socket.send(&connect_packet).unwrap();
    let mut buf: Packet = [0; 77];
    loop {
        match socket.recv(&mut buf) {
            Ok(received) => handle_packet(received, &buf[2..]),
            Err(e) => println!("recv function failed: {:?}", e),
        }
    }
}
