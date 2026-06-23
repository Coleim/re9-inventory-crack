use std::fmt;

pub struct Header {
    magic: [u8; 4],
    version: u32,
    pub encryption_type: u32,
    unknown_2: u32,
}

impl Header {
    pub const SIZE: usize = 4 + 4 + 4 + 4;
    pub fn new(bytes: &[u8]) -> Self {
        Header {
            magic: bytes[0..4].try_into().unwrap(),
            version: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            encryption_type: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            unknown_2: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        }
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "--:: DssFileHeader ::--")?;
        writeln!(
            f,
            "- magic            : {}",
            String::from_utf8_lossy(&self.magic)
        )?;
        writeln!(f, "- version          : {:#x}", self.version)?;
        writeln!(f, "- encryption_type  : {:#x}", self.encryption_type)?;
        writeln!(f, "- unknown_2        : {:#x}", self.unknown_2)?;
        Ok(())
    }
}
