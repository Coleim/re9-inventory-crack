use crate::mandarin;
use std::io::Cursor;

pub const MAGIC: &[u8; 4] = b"DSSS";
pub const VERSION: u32 = 2;
pub const FLAG_MANDARIN: u32 = 0x10;
pub const HEADER_LEN: usize = 16;

pub fn murmur(data: &[u8]) -> u32 {
    murmur3::murmur3_32(&mut Cursor::new(data), 0xffffffff).unwrap()
}

pub struct Header {
    pub version: u32,
    pub flags: u32,
}

pub fn parse_header(file: &[u8]) -> Result<Header, String> {
    if file.len() < HEADER_LEN + 12 {
        return Err("file too small".into());
    }
    if &file[0..4] != MAGIC {
        return Err("bad magic, not a DSSS save".into());
    }
    let version = u32::from_le_bytes(file[4..8].try_into().unwrap());
    let flags = u32::from_le_bytes(file[8..12].try_into().unwrap());
    Ok(Header { version, flags })
}

pub fn decrypted_len(file: &[u8]) -> u64 {
    let len = file.len();
    u64::from_le_bytes(file[len - 12..len - 4].try_into().unwrap())
}

pub fn verify_file_hash(file: &[u8]) -> bool {
    let len = file.len();
    let stored = u32::from_le_bytes(file[len - 4..len].try_into().unwrap());
    murmur(&file[..len - 4]) == stored
}

pub fn payload(file: &[u8]) -> &[u8] {
    let len = file.len();
    &file[HEADER_LEN..len - 4]
}

pub fn decrypt(file: &[u8], steamid: u64) -> Result<Vec<u8>, String> {
    let header = parse_header(file)?;
    if header.flags & FLAG_MANDARIN == 0 {
        return Err(format!("flags {:#x} are not Mandarin", header.flags));
    }
    mandarin::decrypt(payload(file), decrypted_len(file), steamid)
}

pub fn build(plain: &[u8], steamid: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&FLAG_MANDARIN.to_le_bytes());
    out.resize(HEADER_LEN, 0);

    let encrypted = mandarin::encrypt(plain, steamid);
    out.extend_from_slice(&encrypted);
    out.extend_from_slice(&(plain.len() as u64).to_le_bytes());

    while out.len() % 4 != 0 {
        out.push(0);
    }
    let hash = murmur(&out);
    out.extend_from_slice(&hash.to_le_bytes());
    out
}

pub fn crack(file: &[u8]) -> Option<u64> {
    mandarin::crack(
        payload(file),
        decrypted_len(file),
        mandarin::STEAMID_BASE,
        mandarin::STEAMID_COUNT,
    )
}
