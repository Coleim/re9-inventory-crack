use crate::dss::{
    dss_file_footer::DssFileFooter, dss_file_header::DssFileHeader,
    dss_file_segment::DssFileSegment, dss_lime_footer::DssLimeFooter,
    dss_lime_segment::DssLimeSegment,
};
use std::fmt::{self};

pub struct DssFile {
    pub header: DssFileHeader,
    pub segments: Vec<DssFileSegment>,
    pub footer: DssFileFooter,
}

impl DssFile {
    pub fn new(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        let header = DssFileHeader::new(&bytes[0..DssFileHeader::SIZE]);

        if header.encryption_type != 0x10 {
            panic!("Encryption type not supported");
        }
        let footer_start_idx = len - DssLimeFooter::SIZE;
        let footer = DssFileFooter::Lime(DssLimeFooter::new(&bytes[footer_start_idx..]));
        let segments_count =
            (len - DssFileHeader::SIZE - DssLimeFooter::SIZE) / size_of::<&DssLimeSegment>();

        let mut segments: Vec<DssFileSegment> = Vec::with_capacity(segments_count);
        // for i in 0..segments_count {
        segments.push(DssFileSegment::Lime(DssLimeSegment::new(
            &bytes[DssFileHeader::SIZE..footer_start_idx],
        )));
        // }
        DssFile {
            header,
            segments: segments,
            footer,
        }
    }
}

impl fmt::Display for DssFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "--:: DSS File ::--")?;

        for line in format!("{}", self.header).lines() {
            writeln!(f, "  {}", line)?;
        }

        for segment in self.segments.iter() {
            for line in format!("{}", segment).lines() {
                writeln!(f, "  {}", line)?;
            }
        }

        for line in format!("{}", self.footer).lines() {
            writeln!(f, "  {}", line)?;
        }
        Ok(())
    }
}
