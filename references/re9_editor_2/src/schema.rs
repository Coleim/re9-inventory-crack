use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static FIELDS: OnceLock<HashMap<u32, HashSet<u32>>> = OnceLock::new();
pub static RESYNCS: AtomicUsize = AtomicUsize::new(0);

fn load() -> HashMap<u32, HashSet<u32>> {
    let mut map = HashMap::new();
    let text = crate::read_asset(&["re9_fields.tsv", "fields.tsv"], include_str!("../re9_fields.tsv"));
    for line in text.lines() {
        let mut it = line.splitn(2, '\t');
        let (Some(ch), Some(rest)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(ch) = u32::from_str_radix(ch.trim(), 16) else {
            continue;
        };
        let set: HashSet<u32> = rest
            .split(',')
            .filter_map(|h| u32::from_str_radix(h.trim(), 16).ok())
            .collect();
        map.insert(ch, set);
    }
    map
}

static STRUCT_TYPES: OnceLock<HashMap<u64, String>> = OnceLock::new();

fn load_struct_types() -> HashMap<u64, String> {
    let mut map = HashMap::new();
    let text = crate::read_asset(&["re9_structs.tsv", "structs.tsv"], include_str!("../re9_structs.tsv"));
    for line in text.lines() {
        let mut it = line.splitn(2, '\t');
        let (Some(key), Some(ty)) = (it.next(), it.next()) else {
            continue;
        };
        if key.len() == 16 {
            if let Ok(k) = u64::from_str_radix(key.trim(), 16) {
                map.insert(k, ty.trim().to_string());
            }
        }
    }
    map
}

static ENUMS: OnceLock<HashMap<String, HashMap<i64, String>>> = OnceLock::new();

fn load_enums() -> HashMap<String, HashMap<i64, String>> {
    let mut map = HashMap::new();
    let text = crate::read_asset(&["re9_enums.tsv", "enums.tsv"], include_str!("../re9_enums.tsv"));
    for line in text.lines() {
        let mut it = line.splitn(2, '\t');
        let (Some(ty), Some(pairs)) = (it.next(), it.next()) else {
            continue;
        };
        let mut vals = HashMap::new();
        for p in pairs.split(',') {
            if let Some((v, name)) = p.split_once('=') {
                if let Ok(v) = v.trim().parse::<i64>() {
                    vals.insert(v, name.trim().to_string());
                }
            }
        }
        if !vals.is_empty() {
            map.insert(ty.trim().to_string(), vals);
        }
    }
    map
}

pub fn enum_name(enum_type: &str, value: i64) -> Option<&'static str> {
    ENUMS
        .get_or_init(load_enums)
        .get(enum_type)?
        .get(&value)
        .map(|s| s.as_str())
}

pub fn enum_count() -> usize {
    ENUMS.get_or_init(load_enums).len()
}

pub fn field_type(class_hash: u32, field_hash: u32) -> Option<&'static str> {
    let key = ((class_hash as u64) << 32) | field_hash as u64;
    STRUCT_TYPES
        .get_or_init(load_struct_types)
        .get(&key)
        .map(|s| s.as_str())
}

pub fn field_set(class_hash: u32) -> Option<&'static HashSet<u32>> {
    FIELDS.get_or_init(load).get(&class_hash)
}

pub fn count() -> usize {
    FIELDS.get_or_init(load).len()
}

pub fn reset_resyncs() {
    RESYNCS.store(0, Ordering::Relaxed);
}

pub fn resyncs() -> usize {
    RESYNCS.load(Ordering::Relaxed)
}
