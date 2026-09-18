use std::collections::HashMap;
use std::sync::OnceLock;

static TABLE: OnceLock<HashMap<u32, String>> = OnceLock::new();

fn load() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let text = include_str!("../re9_names.tsv");
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

/// Human-readable name for a field/class hash, e.g. `_Stock#955d3f51`, or
/// just the hex hash (`955d3f51`) if it's not in the embedded name table.
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
