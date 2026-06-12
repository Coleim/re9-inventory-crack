use core::fmt;

use crate::dss::dss_lime_footer::DssLimeFooter;

pub enum DssFileFooter {
    Lime(DssLimeFooter),
    // Citrus(DssCitrusFooter),
    // None(DssNoneFooter),
}

impl fmt::Display for DssFileFooter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DssFileFooter::Lime(footer) => write!(f, "{}", footer),
        }
    }
}
