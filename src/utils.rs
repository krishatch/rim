pub fn split_hex_into_bytes(hex: u32) -> (u8, u8, u8) {
    let byte1 = (hex >> 16) as u8; // First byte (most significant)
    let byte2 = (hex >> 8 & 0xFF) as u8; // Second byte
    let byte3 = (hex & 0xFF) as u8; // Third byte (least significant)

    (byte1, byte2, byte3)
}

