// Util functions for bytes, bits and nibbles
use minifb::{Key};

#[inline(always)]
pub fn invalid_instruction(op: u16) {
    panic!("Invalid op code {}", op);
}

#[inline(always)]
pub fn get_key_value(key: &Key) -> u16 {
    *key as u16
}

#[inline(always)]
pub fn get_first_nibble(num: u16) -> u8 {
    ((num & 0xF000) >> 12) as u8
}

#[inline(always)]
pub fn get_second_nibble(num: u16) -> u8 {
    ((num & 0x0F00) >> 8) as u8
}

#[inline(always)]
pub fn get_third_nibble(num: u16) -> u8 {
    ((num & 0x00F0) >> 4) as u8
}

#[inline(always)]
pub fn get_last_nibble(num: u16) -> u8 {
    (num & 0x000F) as u8
}

#[inline(always)]
pub fn get_addr(num: u16) -> u16 {
    num & 0x0FFF
}

#[inline(always)]
pub fn get_last_byte(num: u16) -> u8 {
    (num & 0x00FF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_addr() {
        assert_eq!(get_addr(0xABCD), 0xBCD);
    }

    #[test]
    fn test_get_last_nibble() {
        assert_eq!(get_last_nibble(0xABCD), 0xD);
    }

    #[test]
    fn test_get_third_nibble() {
        assert_eq!(get_third_nibble(0xABCD), 0xC);
    }

    #[test]
    fn test_get_second_nibble() {
        assert_eq!(get_second_nibble(0xABCD), 0xB);
    }

    #[test]
    fn test_get_first_nibble() {
        assert_eq!(get_first_nibble(0xABCD), 0xA);
    }

    #[test]
    fn test_get_last_byte() {
        assert_eq!(get_last_byte(0xABCD), 0xCD);
    }
}