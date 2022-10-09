use std::fs::File;
use std::io;
use std::io::Write;

use crc::{Crc, CRC_32_ISO_HDLC};

use crate::common_output::Controls;
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
    pub battery: u8,
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
            battery: 100,
        }
    }
}

impl DS4Controls {
    fn fill_packet(&self) -> [u8; 7] {
        let mut pkt = [0; 7];
        let (mut red, mut green, mut blue) = (self.red, self.green, self.blue);
        if self.battery == 0 {
            //.load(Ordering::SeqCst) {
            red = 255;
            green = 0;
            blue = 0;
        }

        //pkt[0] = 0x05;
        //pkt[1] = 0x07;
        //pkt[0] = pkt[4] in usb
        eprintln!("Small {}, large {}", self.small, self.large);
        pkt[0] = self.small;
        pkt[1] = self.large;
        pkt[2] = red;
        pkt[3] = green;
        pkt[4] = blue;
        // Time to flash bright (255 = 2.5 seconds)
        pkt[5] = 0; // min(flash_led1, 255)
                    // Time to flash dark (255 = 2.5 seconds)
        pkt[6] = 0; // min(flash_led2, 255)
                    //pkt[19] = self.volume_l;
                    //pkt[20] = self.volume_r;
                    //pkt[21] = 0x49; // magic
                    //pkt[22] = self.volume_speaker;
                    //pkt[23] = 0x85; //magic
        pkt
    }
}

impl Controls for DS4Controls {
    fn write_packet_usb(&self, f_write: &mut File) -> io::Result<()> {
        let mut pkt = [0; 32];
        pkt[4..11].copy_from_slice(&self.fill_packet());
        pkt[0] = 0x05;
        pkt[1] = 0x07;
        let count = f_write.write(&pkt)?;
        assert_eq!(count, 32);
        f_write.flush()
    }

    fn write_packet_bt(&mut self, f_write: &mut File) -> io::Result<()> {
        let mut pkt = [0; 78];
        pkt[6..13].copy_from_slice(&self.fill_packet());
        pkt[0] = 0x11;
        pkt[1] = 0xC0 | self.latency;
        pkt[3] = 0x07; //magic
        pkt[21] = self.volume_l;
        pkt[22] = self.volume_r;
        pkt[23] = 0x49; // magic
        pkt[24] = self.volume_speaker;
        pkt[25] = 0x85; //magic
        let crc = checksum_ds4(&pkt[0..74]);
        pkt[74..78].copy_from_slice(&crc);
        let count = f_write.write(&pkt)?;
        assert_eq!(count, 78);
        f_write.flush()
    }
}
