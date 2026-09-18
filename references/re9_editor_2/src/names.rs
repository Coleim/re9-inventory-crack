use std::collections::HashMap;
use std::sync::OnceLock;

static TABLE: OnceLock<HashMap<u32, String>> = OnceLock::new();

fn load() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let text = crate::read_asset(&["re9_names.tsv", "names.tsv"], include_str!("../re9_names.tsv"));
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.splitn(2, |c| c == '\t' || c == ' ');
        let (Some(h), Some(name)) = (it.next(), it.next()) else {
            continue;
        };
        let h = h.trim_start_matches("0x");
        if let Ok(hash) = u32::from_str_radix(h, 16) {
            map.insert(hash, name.trim().to_string());
        }
    }
    map
}

pub fn name_for(hash: u32) -> String {
    let table = TABLE.get_or_init(load);
    match table.get(&hash) {
        Some(n) => format!("{n}#{hash:08x}"),
        None => format!("{hash:08x}"),
    }
}

pub fn count() -> usize {
    TABLE.get_or_init(load).len()
}

pub fn murmur3(name: &str) -> u32 {
    let data = name.as_bytes();
    let mut h: u32 = 0xffffffff;
    let nblocks = data.len() / 4;
    for i in 0..nblocks {
        let mut k = u32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap());
        k = k.wrapping_mul(0xcc9e2d51);
        k = k.rotate_left(15);
        k = k.wrapping_mul(0x1b873593);
        h ^= k;
        h = h.rotate_left(13);
        h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    }
    let tail = &data[nblocks * 4..];
    let mut k: u32 = 0;
    for (i, &b) in tail.iter().enumerate() {
        k ^= (b as u32) << (8 * i);
    }
    if !tail.is_empty() {
        k = k.wrapping_mul(0xcc9e2d51);
        k = k.rotate_left(15);
        k = k.wrapping_mul(0x1b873593);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h
}
