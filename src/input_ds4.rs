use std::fs::File;
use std::io;
use std::io::prelude::*;

use crate::common_input::{DS4PacketInner, Packet, PACKET_LEN_BT, PACKET_LEN_USB};

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
        (self.inner[32] & 0xF) * 10
    }
    fn to_ds4_packet(&self) -> DS4PacketInner {
        let mut res: DS4PacketInner = [0; PACKET_LEN_USB];
        res.copy_from_slice(&self.inner[2..PACKET_LEN_USB + 2]);
        res
    }
    fn is_valid(&self) -> bool {
        self.inner[0] == 0x11
    }
    fn get_size(&self) -> usize {
        PACKET_LEN_BT
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
        (self.inner[30] & 0xF) * 10
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
}
