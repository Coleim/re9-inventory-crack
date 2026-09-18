use crate::dsss::header::Header;
use std::fmt::{self};

const MURMUR_LEN: usize = 4;
const DECRYPTED_DATA_LEN_SIZE: usize = 8;

pub struct File {
    pub header: Header,
    pub payload: Vec<u8>,
    pub murmur_hash_3_signature: u32,
    pub decrypted_data_len: u64,
}

impl File {
    pub fn new(bytes: Vec<u8>) -> Self {
        let len = bytes.len();

        let header = Header::new(&bytes[0..Header::SIZE]);
        if header.encryption_type != 0x10 {
            panic!("Encryption type not supported");
        }

        let murmur = u32::from_le_bytes(bytes[len - MURMUR_LEN..].try_into().unwrap());
        let decrypted_data_len = u64::from_le_bytes(
            bytes[len - DECRYPTED_DATA_LEN_SIZE - MURMUR_LEN..len - MURMUR_LEN]
                .try_into()
                .unwrap(),
        );

        File {
            header,
            payload: bytes[Header::SIZE..len - MURMUR_LEN].try_into().unwrap(),
            murmur_hash_3_signature: murmur,
            decrypted_data_len,
        }
    }

    pub fn first_ciphertext_bytes(&self) -> [u8; 8] {
        self.payload[0..8].try_into().unwrap()
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "--:: DSS File ::--")?;

        for line in format!("{}", self.header).lines() {
            writeln!(f, "  {}", line)?;
        }

        writeln!(f, "-  decrypted_data_len: {}", self.decrypted_data_len)?;
        writeln!(
            f,
            "-  murmur_hash_3_signature: {}",
            self.murmur_hash_3_signature
        )?;

        Ok(())
    }
}
