use std::fs::File;
use std::io;
use std::io::Write;

use crc::{Crc, CRC_32_ISO_HDLC};

use crate::common_output::Controls;
const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Debug)]
pub struct DSenseControls {
    pub large: u8,
    pub small: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub battery: u8,
    seq: u8,
}

fn checksum_dsense(packet: &[u8]) -> [u8; 4] {
    let mut full_packet = [0u8; 75];
    full_packet[0] = 0xA2;
    full_packet[1..].copy_from_slice(packet);
    let hasher = CRC.checksum(&full_packet);
    hasher.to_le_bytes()
}

impl Default for DSenseControls {
    fn default() -> Self {
        Self {
            large: 0,
            small: 0,
            red: 0,
            green: 0,
            blue: 255,
            battery: 100,
            seq: 0,
        }
    }
}

fn get_player_led_from_battery(battery: u8) -> u8 {
    if battery >= 90 {
        //return 0x01 | 0x02 | 0x08 | 0x10;
        return 0x1F;
    }
    if battery >= 70 {
        //return 0x01 | 0x02 | 0x08;
        return 0x1B;
    }
    if battery >= 50 {
        //return 0x01 | 0x02;
        return 0x15;
    }
    if battery >= 20 {
        //return 0x01;
        return 0x06;
    }
    if battery >= 10 {
        //return 0x01 | 0x02 | 0x04;
        return 0x04;
    }
    0
}

impl DSenseControls {
    fn fill_packet(&self) -> [u8; 47] {
        let mut pkt = [0; 47];
        pkt[0] = 0x0F;
        pkt[1] = 0x55;
        pkt[2] = self.small;
        pkt[3] = self.large;
        //pkt[8] = muteLEDByte;
        pkt[38] = 0x05;
        //pkt[41] = 0x02;
        pkt[42] = 0x02;
        pkt[43] = get_player_led_from_battery(self.battery);
        pkt[44] = self.red;
        pkt[45] = self.green;
        pkt[46] = self.blue;
        pkt
    }
}

impl Controls for DSenseControls {
    fn write_packet_usb(&self, f_write: &mut File) -> io::Result<()> {
        let mut pkt = [0; 63];
        pkt[1..48].copy_from_slice(&self.fill_packet());
        pkt[0] = 0x02;
        let count = f_write.write(&pkt)?;
        assert_eq!(count, 63);
        f_write.flush()
    }

    fn write_packet_bt(&mut self, f_write: &mut File) -> io::Result<()> {
        let mut pkt = [0; 78];
        pkt[3..50].copy_from_slice(&self.fill_packet());
        pkt[0] = 0x31;
        pkt[1] = self.seq << 4;
        pkt[2] = 0x10;
        self.seq += 1;
        if self.seq == 16 {
            self.seq = 0;
        }
        let crc = checksum_dsense(&pkt[0..74]);
        pkt[74..78].copy_from_slice(&crc);
        println!("Packet {:?}", pkt);
        let count = f_write.write(&pkt)?;
        assert_eq!(count, 78);
        f_write.flush()
    }
}
