use std::fs::File;
use std::io;

pub const PACKET_LEN_USB: usize = 64;
pub const PACKET_LEN_BT: usize = 78;

pub type DS4PacketInner = [u8; PACKET_LEN_USB];

pub trait Packet {
    fn read(&mut self, f: &mut File) -> io::Result<()>;
    fn battery_capacity(&self) -> u8;
    fn to_ds4_packet(&self) -> DS4PacketInner;
    fn is_valid(&self) -> bool;
    fn get_size(&self) -> usize;
}
