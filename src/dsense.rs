use std::fs::File;
use std::io;
use std::io::prelude::*;

use crate::common::{DS4PacketInner, Packet, PACKET_LEN_BT, PACKET_LEN_USB};

pub struct DSensePacketBT {
    inner: [u8; PACKET_LEN_BT],
}
pub struct DSensePacketUSB {
    inner: [u8; PACKET_LEN_USB],
}

impl DSensePacketBT {
    pub fn new() -> DSensePacketBT {
        DSensePacketBT {
            inner: [0; PACKET_LEN_BT],
        }
    }
}

impl Packet for DSensePacketBT {
    fn read(&mut self, f: &mut File) -> io::Result<()> {
        let count = f.read(&mut self.inner)?;
        assert_eq!(count, self.get_size());
        assert!(self.is_valid());
        Ok(())
    }
    fn battery_capacity(&self) -> u8 {
        100
    }
    fn to_ds4_packet(&self) -> DS4PacketInner {
        let mut new_packet: DS4PacketInner = [0; PACKET_LEN_USB];
        new_packet[1] = self.inner[2];
        new_packet[2] = self.inner[3];
        new_packet[3] = self.inner[4];
        new_packet[4] = self.inner[5];
        new_packet[5] = self.inner[9];
        new_packet[6] = self.inner[10];
        new_packet[8] = self.inner[6];
        new_packet[9] = self.inner[7];
        new_packet
    }
    fn is_valid(&self) -> bool {
        self.inner[0] == 0x31
    }
    fn get_size(&self) -> usize {
        PACKET_LEN_BT
    }
    fn control(&self, writer: &mut File) -> io::Result<()> {
        Ok(())
    }
}

impl DSensePacketUSB {
    pub fn new() -> DSensePacketUSB {
        DSensePacketUSB {
            inner: [0; PACKET_LEN_USB],
        }
    }
}

impl Packet for DSensePacketUSB {
    fn read(&mut self, f: &mut File) -> io::Result<()> {
        let count = f.read(&mut self.inner)?;
        assert_eq!(count, self.get_size());
        assert!(self.is_valid());
        Ok(())
    }
    fn battery_capacity(&self) -> u8 {
        100
    }
    fn to_ds4_packet(&self) -> DS4PacketInner {
        let mut new_packet: DS4PacketInner = [0; PACKET_LEN_USB];
        new_packet[1] = self.inner[1];
        new_packet[2] = self.inner[2];
        new_packet[3] = self.inner[3];
        new_packet[4] = self.inner[4];
        new_packet[5] = self.inner[8];
        new_packet[6] = self.inner[9];
        new_packet[8] = self.inner[5];
        new_packet[9] = self.inner[6];
        new_packet
    }
    fn is_valid(&self) -> bool {
        self.inner[0] == 0x01
    }
    fn get_size(&self) -> usize {
        PACKET_LEN_USB
    }
    fn control(&self, writer: &mut File) -> io::Result<()> {
        Ok(())
    }
}
