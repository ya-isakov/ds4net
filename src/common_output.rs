use std::fs::File;
use std::io;

pub trait Controls {
    fn write_packet_usb(&self, f_write: &mut File) -> io::Result<()>;
    fn write_packet_bt(&mut self, f_write: &mut File) -> io::Result<()>;
}
