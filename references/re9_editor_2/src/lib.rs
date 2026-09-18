pub mod bignum;
pub mod dsss;
pub mod edit;
pub mod elgamal;
pub mod error;
pub mod mandarin;
pub mod names;
pub mod rsz;
pub mod schema;
pub mod splitmix;

pub use error::{Error, Result};

pub fn read_asset(candidates: &[&str], embedded: &'static str) -> std::borrow::Cow<'static, str> {
    for p in candidates {
        if let Ok(t) = std::fs::read_to_string(p) {
            return std::borrow::Cow::Owned(t);
        }
    }
    std::borrow::Cow::Borrowed(embedded)
}
