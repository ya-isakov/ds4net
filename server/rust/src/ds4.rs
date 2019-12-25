use crc::crc32;

const DEFAULT_LATENCY: u8 = 4;

#[derive(Debug)]
pub struct DS4Controls {
    pub large: u8,
    pub small: u8,
    latency: u8,
    red: u8,
    green: u8,
    blue: u8,
    volume_l: u8,
    volume_r: u8,
    volume_speaker: u8,
}

fn transform_u32_to_array_of_u8(x: u32) -> [u8; 4] {
    let b0: u8 = ((x >> 24) & 0xff) as u8;
    let b1: u8 = ((x >> 16) & 0xff) as u8;
    let b2: u8 = ((x >> 8) & 0xff) as u8;
    let b3: u8 = (x & 0xff) as u8;
    [b3, b2, b1, b0]
}

fn checksum(packet: &[u8]) -> [u8; 4] {
    let mut full_packet = [0u8; 75];
    full_packet[0] = 0xA2;
    full_packet[1..].copy_from_slice(packet);
    let hasher = crc32::checksum_ieee(&full_packet);
    transform_u32_to_array_of_u8(hasher.to_le())
}

impl Default for DS4Controls {
    fn default() -> Self {
        DS4Controls {
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
    fn fill_packet(self: &Self) -> [u8; 78] {
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

    pub fn make_packet_with_checksum(self: &Self) -> [u8; 78] {
        let mut pkt = self.fill_packet();
        let crc = checksum(&pkt[0..74]);
        pkt[74..78].copy_from_slice(&crc);
        pkt
    }
}
