#[derive(Clone, Copy)]
pub enum ValType {
    U8,
    U16,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
}

impl ValType {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(match s {
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            other => return Err(format!("unknown type {other} (u8/u16/u32/u64/i32/i64/f32/f64)")),
        })
    }

    pub fn size(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }

    pub fn read(self, b: &[u8]) -> String {
        match self {
            Self::U8 => b[0].to_string(),
            Self::U16 => u16::from_le_bytes(b[..2].try_into().unwrap()).to_string(),
            Self::U32 => u32::from_le_bytes(b[..4].try_into().unwrap()).to_string(),
            Self::U64 => u64::from_le_bytes(b[..8].try_into().unwrap()).to_string(),
            Self::I32 => i32::from_le_bytes(b[..4].try_into().unwrap()).to_string(),
            Self::I64 => i64::from_le_bytes(b[..8].try_into().unwrap()).to_string(),
            Self::F32 => f32::from_le_bytes(b[..4].try_into().unwrap()).to_string(),
            Self::F64 => f64::from_le_bytes(b[..8].try_into().unwrap()).to_string(),
        }
    }

    pub fn encode(self, v: &str) -> Result<Vec<u8>, String> {
        let e = |x: &str| format!("invalid value {x}");
        Ok(match self {
            Self::U8 => vec![v.parse::<u8>().map_err(|_| e(v))?],
            Self::U16 => v.parse::<u16>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
            Self::U32 => v.parse::<u32>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
            Self::U64 => v.parse::<u64>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
            Self::I32 => v.parse::<i32>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
            Self::I64 => v.parse::<i64>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
            Self::F32 => v.parse::<f32>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
            Self::F64 => v.parse::<f64>().map_err(|_| e(v))?.to_le_bytes().to_vec(),
        })
    }
}

pub fn parse_offset(s: &str) -> Result<usize, String> {
    let s = s.trim();
    let r = if let Some(h) = s.strip_prefix("0x") {
        usize::from_str_radix(h, 16)
    } else {
        s.parse::<usize>()
    };
    r.map_err(|_| format!("invalid offset {s}"))
}

pub fn utf16_strings(d: &[u8], minlen: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < d.len() {
        if (0x20..0x7f).contains(&d[i]) && d[i + 1] == 0 {
            let start = i;
            let mut s = String::new();
            while i + 1 < d.len() && (0x20..0x7f).contains(&d[i]) && d[i + 1] == 0 {
                s.push(d[i] as char);
                i += 2;
            }
            if s.len() >= minlen {
                out.push((start, s));
            }
        } else {
            i += 1;
        }
    }
    out
}

pub fn hexdump(d: &[u8], off: usize, n: usize) -> String {
    let mut s = String::new();
    let end = (off + n).min(d.len());
    let mut i = off & !0xf;
    while i < end {
        s.push_str(&format!("{i:08x}  "));
        for j in 0..16 {
            if i + j < d.len() {
                s.push_str(&format!("{:02x} ", d[i + j]));
            } else {
                s.push_str("   ");
            }
        }
        s.push(' ');
        for j in 0..16 {
            if i + j < d.len() {
                let c = d[i + j];
                s.push(if (0x20..0x7f).contains(&c) { c as char } else { '.' });
            }
        }
        s.push('\n');
        i += 16;
    }
    s
}

pub fn find_u32(d: &[u8], needle: u32) -> Vec<usize> {
    let n = needle.to_le_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= d.len() {
        if d[i..i + 4] == n {
            out.push(i);
        }
        i += 1;
    }
    out
}
