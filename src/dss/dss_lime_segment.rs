use core::fmt;

pub struct LimePublicKeyHeader {}
pub struct LimePublicKeyFragmentsBank {}
pub struct DssLimeSegment {
    key_fragments_bank: [LimePublicKeyFragmentsBank; 4],
    segment_data: [u8; 0x1000],
    segment_checksum: [u64; 4],
}

impl DssLimeSegment {
    pub fn new(bytes: &[u8]) -> Self {
        DssLimeSegment {}
    }
}
impl fmt::Display for DssLimeSegment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "--:: DssLimeSegment ::--")?;
        Ok(())
    }
}
