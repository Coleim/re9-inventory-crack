use core::fmt;

use crate::dss::dss_lime_segment::DssLimeSegment;

pub enum DssFileSegment {
    Lime(DssLimeSegment),
}
impl fmt::Display for DssFileSegment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DssFileSegment::Lime(segment) => write!(f, "{}", segment),
        }
    }
}
