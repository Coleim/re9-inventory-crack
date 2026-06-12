use std::fmt;

const SALT_END: usize = 128;
const LEN_END: usize = SALT_END + 8;

pub struct DssLimeFooter {
    pub salt: [u8; SALT_END],
    decrypted_data_length: u64,
    murmure_hash_3_signature: u32,
}

impl DssLimeFooter {
    pub const SIZE: usize = 128 + 8 + 4; // 140 bytes
    pub fn new(bytes: &[u8]) -> Self {
        DssLimeFooter {
            salt: bytes[..SALT_END].try_into().unwrap(),
            decrypted_data_length: u64::from_le_bytes(bytes[SALT_END..LEN_END].try_into().unwrap()),
            murmure_hash_3_signature: u32::from_le_bytes(bytes[LEN_END..].try_into().unwrap()),
        }
    }
}

impl fmt::Display for DssLimeFooter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "--:: DssLimeFooter ::--")?;
        writeln!(
            f,
            "- salt                      : {}",
            String::from_utf8_lossy(&self.salt)
        )?;
        writeln!(
            f,
            "- decrypted_data_length     : {:#x} ({})",
            self.decrypted_data_length, self.decrypted_data_length
        )?;
        writeln!(
            f,
            "- murmure_hash_3_signature  : {:#x} ({})",
            self.murmure_hash_3_signature, self.murmure_hash_3_signature
        )?;
        Ok(())
    }
}
