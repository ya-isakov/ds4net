use std::fs::File;
use std::io;

pub const DS4_PACKET_LEN_USB: usize = 64;

pub type DS4PacketInner = [u8; DS4_PACKET_LEN_USB];

pub trait Packet {
    fn read(&mut self, f: &mut File) -> io::Result<()>;
    fn battery_capacity(&self) -> u8;
    fn to_ds4_packet(&self) -> DS4PacketInner;
    fn is_valid(&self) -> bool;
    fn get_size(&self) -> usize;
    fn control(&self, writer: &mut File) -> io::Result<()>;
}
