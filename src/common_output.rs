use std::fs::File;
use std::io;

use crc::{Crc, CRC_32_ISO_HDLC};

pub trait Controls {
    fn set_color(&mut self, r: u8, g: u8, b: u8);
    fn set_rumble(&mut self, large: u8, small: u8);
    fn set_battery(&mut self, level: u8);
    fn write_packet_usb(&self, f_write: &mut File) -> io::Result<()>;
    fn write_packet_bt(&mut self, f_write: &mut File) -> io::Result<()>;
}

const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub fn calculate_checksum_bt(packet: &[u8]) -> [u8; 4] {
    let mut full_packet = [0u8; 75];
    full_packet[0] = 0xA2;
    full_packet[1..].copy_from_slice(packet);
    let hasher = CRC.checksum(&full_packet);
    hasher.to_le_bytes()
}
