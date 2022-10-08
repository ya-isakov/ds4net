use std::fs::File;
use std::io;
use std::io::prelude::*;

use crate::common::{DS4PacketInner, Packet, PACKET_LEN_BT, PACKET_LEN_USB};

use crc::{Crc, CRC_32_ISO_HDLC};

const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
const DEFAULT_LATENCY: u8 = 4;

#[derive(Debug)]
pub struct DS4Controls {
    pub large: u8,
    pub small: u8,
    latency: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    volume_l: u8,
    volume_r: u8,
    volume_speaker: u8,
}

pub trait Controls {
    fn make_packet_with_checksum(self) -> [u8; 78];
}

fn checksum_ds4(packet: &[u8]) -> [u8; 4] {
    let mut full_packet = [0u8; 75];
    full_packet[0] = 0xA2;
    full_packet[1..].copy_from_slice(packet);
    let hasher = CRC.checksum(&full_packet);
    hasher.to_le_bytes()
}

impl Default for DS4Controls {
    fn default() -> Self {
        Self {
            large: 0,
            small: 0,
            latency: DEFAULT_LATENCY,
            red: 0,
            green: 0,
            blue: 255,
            volume_l: 0,
            volume_r: 0,
            volume_speaker: 0,
        }
    }
}

impl DS4Controls {
    fn fill_packet(self) -> [u8; 78] {
        let mut pkt = [0u8; 78];
        pkt[0] = 0x11;
        pkt[1] = 0xC0 | self.latency;
        pkt[3] = 0x07;
        pkt[6] = self.small;
        pkt[7] = self.large;
        pkt[8] = self.red;
        pkt[9] = self.green;
        pkt[10] = self.blue;
        // Time to flash bright (255 = 2.5 seconds)
        pkt[11] = 0; // min(flash_led1, 255)
                     // Time to flash dark (255 = 2.5 seconds)
        pkt[12] = 0; // min(flash_led2, 255)
        pkt[21] = self.volume_l;
        pkt[22] = self.volume_r;
        pkt[23] = 0x49; // magic
        pkt[24] = self.volume_speaker;
        pkt[25] = 0x85; //magic
        pkt
    }
}

impl Controls for DS4Controls {
    fn make_packet_with_checksum(self) -> [u8; 78] {
        let mut pkt = self.fill_packet();
        let crc = checksum_ds4(&pkt[0..74]);
        pkt[74..78].copy_from_slice(&crc);
        pkt
    }
}

pub struct DS4PacketBT {
    inner: [u8; PACKET_LEN_BT],
}
pub struct DS4PacketUSB {
    inner: DS4PacketInner,
}

impl DS4PacketBT {
    pub fn new() -> DS4PacketBT {
        DS4PacketBT {
            inner: [0; PACKET_LEN_BT],
        }
    }
}

impl Packet for DS4PacketBT {
    fn read(&mut self, f: &mut File) -> io::Result<()> {
        let count = f.read(&mut self.inner)?;
        assert_eq!(count, self.get_size());
        assert!(self.is_valid());
        Ok(())
    }
    fn battery_capacity(&self) -> u8 {
        self.inner[54] & 0xF
    }
    fn to_ds4_packet(&self) -> DS4PacketInner {
        let mut res: DS4PacketInner = [0; PACKET_LEN_USB];
        res.copy_from_slice(&self.inner[0..PACKET_LEN_USB]);
        res
    }
    fn is_valid(&self) -> bool {
        //self.inner[0] == 0x11
        true
    }
    fn get_size(&self) -> usize {
        PACKET_LEN_BT
    }
    fn control(&self, writer: &mut File) -> io::Result<()> {
        //    let pkt = self.make_packet_with_checksum();
        //    match writer.write(&pkt) {
        //        Ok(count) => assert_eq!(count, self.get_size()),
        //        Err(e) => return Err(e),
        //    };
        //    writer.flush()
        Ok(())
    }
}

impl DS4PacketUSB {
    pub fn new() -> DS4PacketUSB {
        DS4PacketUSB {
            inner: [0; PACKET_LEN_USB],
        }
    }
}

impl Packet for DS4PacketUSB {
    fn read(&mut self, f: &mut File) -> io::Result<()> {
        let count = f.read(&mut self.inner)?;
        assert_eq!(count, self.get_size());
        assert!(self.is_valid());
        Ok(())
    }
    fn battery_capacity(&self) -> u8 {
        self.inner[54] & 0xF
    }
    fn to_ds4_packet(&self) -> DS4PacketInner {
        self.inner
    }
    fn is_valid(&self) -> bool {
        self.inner[0] == 0x1
    }
    fn get_size(&self) -> usize {
        PACKET_LEN_USB
    }
    fn control(&self, writer: &mut File) -> io::Result<()> {
        //    let pkt = self.make_packet_with_checksum();
        //    match writer.write(&pkt) {
        //        Ok(count) => assert_eq!(count, self.get_size()),
        //        Err(e) => return Err(e),
        //    };
        //    writer.flush()
        Ok(())
    }
}
