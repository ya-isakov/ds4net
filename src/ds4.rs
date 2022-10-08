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
