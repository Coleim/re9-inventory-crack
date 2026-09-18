//! Generic, self-describing RSZ tree parser + in-place editor.
//!
//! Ported from the reference re9_editor project. Unlike a byte-signature
//! scan, this walks the actual RSZ encoding (hash/type/size-prefixed
//! fields), using `crate::schema` (built from `re9_fields.tsv` /
//! `re9_structs.tsv`) to detect and resync from desyncs and to get type
//! hints for opaque `Struct` blobs (e.g. `via.vec3`).
//!
//! Editing is done in place: since array/class sizes never change, we only
//! ever overwrite fixed-width scalar bytes at their already-known offset in
//! the decrypted buffer - no re-serialization needed.

pub struct Reader<'a> {
    pub d: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        Self { d, pos: 0 }
    }
    pub fn align(&mut self, n: usize) {
        if n > 1 {
            self.pos = self.pos.div_ceil(n) * n;
        }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.d.len() {
            return Err(format!("eof at {:#x} (+{n})", self.pos));
        }
        let s = &self.d[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}

pub fn type_name(t: i32) -> &'static str {
    match t {
        -1 => "Array",
        0 => "Unknown",
        1 => "Enum",
        2 => "Bool",
        3 => "S8",
        4 => "U8",
        5 => "S16",
        6 => "U16",
        7 => "S32",
        8 => "U32",
        9 => "S64",
        0xa => "U64",
        0xb => "F32",
        0xc => "F64",
        0xd => "C8",
        0xe => "C16",
        0xf => "String",
        0x10 => "Struct",
        0x11 => "Class",
        _ => "?",
    }
}

/// Human-readable display name for a field/class hash, backed by the
/// embedded `re9_names.tsv` (see [`crate::names`]); falls back to the bare
/// hex hash if the name isn't in the table.
pub fn name_for(hash: u32) -> String {
    crate::names::name_for(hash)
}

pub enum Value {
    Scalar {
        off: usize,
        ftype: i32,
        width: u8,
        text: String,
    },
    Str {
        off: usize,
        s: String,
    },
    StructBytes {
        off: usize,
        bytes: Vec<u8>,
    },
    Array {
        member_type: i32,
        items: Vec<Value>,
    },
    Class(Class),
}

pub struct Field {
    pub hash: u32,
    pub ftype: i32,
    pub value: Value,
}

pub struct Class {
    pub hash: u32,
    pub fields: Vec<Field>,
    pub truncated: Option<String>,
}

fn read_scalar_text(r: &mut Reader, ftype: i32, size: u32) -> Result<String, String> {
    if size != 1 {
        r.align(size as usize);
    }
    let off = r.pos;
    let text = match ftype {
        2 => format!("{}", r.u8()? != 0),
        3 => format!("{}", r.u8()? as i8),
        4 | 0xd => format!("{}", r.u8()?),
        5 => format!("{}", r.u16()? as i16),
        6 | 0xe => format!("{}", r.u16()?),
        7 => format!("{}", r.i32()?),
        8 => format!("{}", r.u32()?),
        9 => format!("{}", r.u64()? as i64),
        0xa => format!("{}", r.u64()?),
        0xb => format!("{}", f32::from_bits(r.u32()?)),
        0xc => format!("{}", f64::from_bits(r.u64()?)),
        1 => match size {
            1 => format!("{}", r.u8()? as i8),
            2 => format!("{}", r.u16()? as i16),
            4 => format!("{}", r.i32()?),
            8 => format!("{}", r.u64()? as i64),
            _ => return Err(format!("bad enum size {size}")),
        },
        _ => return Err(format!("unexpected scalar type {ftype} @{:#x}", r.pos)),
    };
    let _ = off;
    Ok(text)
}

fn read_value(r: &mut Reader, ftype: i32) -> Result<Value, String> {
    match ftype {
        -1 => read_array(r),
        0x11 => Ok(Value::Class(read_class(r)?)),
        0xf => {
            r.align(4);
            let size = r.u32()?;
            let off = r.pos;
            let mut s = String::new();
            for _ in 0..size {
                let c = r.u16()?;
                if c != 0 {
                    s.push(char::from_u32(c as u32).unwrap_or('\u{fffd}'));
                }
            }
            Ok(Value::Str { off, s })
        }
        0x10 => {
            r.align(4);
            let size = r.u32()?;
            if size != 1 {
                r.align(size as usize);
            }
            let off = r.pos;
            let bytes = r.d[r.pos..(r.pos + size as usize).min(r.d.len())].to_vec();
            r.pos += size as usize;
            Ok(Value::StructBytes { off, bytes })
        }
        _ => {
            r.align(4);
            let size = r.u32()?;
            let off_align = r.pos;
            let text = read_scalar_text(r, ftype, size)?;
            let off = if size != 1 {
                (off_align + size as usize - 1) & !(size as usize - 1)
            } else {
                off_align
            };
            Ok(Value::Scalar {
                off,
                ftype,
                width: size as u8,
                text,
            })
        }
    }
}

fn read_array(r: &mut Reader) -> Result<Value, String> {
    r.align(4);
    let member_type = r.i32()?;
    let member_size = r.u32()?;
    let len = r.u32()?;
    let array_type = r.i32()?;
    if len > 1_000_000 {
        return Err(format!("array len too big: {len}"));
    }
    if array_type == 1 {
        let marker = r.u32()?;
        if marker == 0xffeeffee {
            for _ in 0..len {
                let _ = r.u32()?;
            }
        } else {
            r.pos -= 4;
        }
    }
    let mut items = Vec::new();
    for _ in 0..len {
        let v = match array_type {
            0 => match member_type {
                0xf => read_value(r, 0xf)?,
                0x10 => {
                    if member_size != 1 {
                        r.align(member_size as usize);
                    }
                    let off = r.pos;
                    let end = (r.pos + member_size as usize).min(r.d.len());
                    let bytes = r.d[r.pos..end].to_vec();
                    r.pos += member_size as usize;
                    Value::StructBytes { off, bytes }
                }
                _ => {
                    if member_size != 1 {
                        r.align(member_size as usize);
                    }
                    let off = r.pos;
                    let text = read_scalar_text(r, member_type, member_size)?;
                    Value::Scalar {
                        off,
                        ftype: member_type,
                        width: member_size as u8,
                        text,
                    }
                }
            },
            _ => Value::Class(read_class(r)?),
        };
        let stop = is_truncated(&v);
        items.push(v);
        if stop {
            break;
        }
    }
    r.align(4);
    Ok(Value::Array { member_type, items })
}

fn peek_u32(d: &[u8], p: usize) -> Option<u32> {
    d.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn plausible_ftype(t: i32) -> bool {
    t == -1 || (0..=0x11).contains(&t)
}

fn resync_target(
    d: &[u8],
    start: usize,
    set: &std::collections::HashSet<u32>,
    consumed: &std::collections::HashSet<u32>,
) -> Option<usize> {
    let end = (start + 192).min(d.len().saturating_sub(8));
    let mut p = start;
    while p <= end {
        if let Some(h) = peek_u32(d, p) {
            if set.contains(&h) && !consumed.contains(&h) {
                if let Some(t) = peek_u32(d, p + 4).map(|v| v as i32) {
                    if plausible_ftype(t) {
                        return Some(p);
                    }
                }
            }
        }
        p += 4;
    }
    None
}

fn is_truncated(v: &Value) -> bool {
    match v {
        Value::Class(c) => {
            c.truncated.is_some() || c.fields.last().map(|f| is_truncated(&f.value)).unwrap_or(false)
        }
        Value::Array { items, .. } => items.last().map(is_truncated).unwrap_or(false),
        _ => false,
    }
}

fn read_class(r: &mut Reader) -> Result<Class, String> {
    let num_fields = r.u32()?;
    let hash = r.u32()?;
    if num_fields > 100_000 {
        return Err(format!("class num_fields too big: {num_fields}"));
    }
    let schema = crate::schema::field_set(hash);
    let mut consumed = std::collections::HashSet::new();
    let mut fields = Vec::new();
    let mut truncated = None;
    for fi in 0..num_fields {
        if let Some(set) = schema {
            let cur = peek_u32(r.d, r.pos);
            if cur.map(|h| !set.contains(&h)).unwrap_or(false) {
                match resync_target(r.d, r.pos, set, &consumed) {
                    Some(np) => {
                        r.pos = np;
                        crate::schema::RESYNCS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    None => {
                        truncated = Some(format!(
                            "field {fi}/{num_fields} @{:#x}: desync, no schema resync target",
                            r.pos
                        ));
                        break;
                    }
                }
            }
        }
        let fstart = r.pos;
        let fhash = match r.u32() {
            Ok(h) => h,
            Err(e) => {
                truncated = Some(format!("field {fi}/{num_fields}: {e}"));
                break;
            }
        };
        let ftype = r.i32()?;
        let value = match read_value(r, ftype) {
            Ok(v) => v,
            Err(e) => {
                truncated = Some(format!(
                    "field {fi}/{num_fields} @{fstart:#x} (hash {fhash:08x} type {ftype}): {e}"
                ));
                break;
            }
        };
        r.align(4);
        let stop = is_truncated(&value);
        consumed.insert(fhash);
        fields.push(Field {
            hash: fhash,
            ftype,
            value,
        });
        if stop {
            truncated = Some(format!("nested truncation at field {fi}/{num_fields}"));
            break;
        }
    }
    Ok(Class {
        hash,
        fields,
        truncated,
    })
}

fn struct_floats(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() % 4 != 0 || bytes.len() > 64 {
        return None;
    }
    let parts: Vec<String> = bytes
        .chunks(4)
        .map(|c| format!("{}", f32::from_le_bytes(c.try_into().unwrap())))
        .collect();
    Some(parts.join(", "))
}

fn fmtf(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    let a = x.abs();
    if !x.is_finite() || a < 1e-4 || a >= 1e7 {
        format!("{x:.4e}")
    } else {
        let s = format!("{x:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn rd_f32(b: &[u8], i: usize) -> f32 {
    f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
}
fn rd_f64(b: &[u8], i: usize) -> f64 {
    f64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap())
}

fn guid_str(b: &[u8]) -> String {
    let d1 = u32::from_le_bytes(b[0..4].try_into().unwrap());
    let d2 = u16::from_le_bytes(b[4..6].try_into().unwrap());
    let d3 = u16::from_le_bytes(b[6..8].try_into().unwrap());
    format!(
        "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

pub fn enum_label(hint: Option<&str>, text: &str) -> Option<&'static str> {
    let ty = hint?;
    let val: i64 = text.trim().parse().ok()?;
    crate::schema::enum_name(ty, val)
}

pub fn decode_struct(type_name: &str, b: &[u8]) -> Option<String> {
    let floats = |n: usize| -> Option<String> {
        if b.len() < n * 4 {
            return None;
        }
        Some((0..n).map(|i| fmtf(rd_f32(b, i) as f64)).collect::<Vec<_>>().join(", "))
    };
    match type_name {
        "via.Position" if b.len() >= 24 => {
            Some(format!("({}, {}, {})", fmtf(rd_f64(b, 0)), fmtf(rd_f64(b, 1)), fmtf(rd_f64(b, 2))))
        }
        "via.vec2" | "via.Float2" | "via.Size" => floats(2).map(|s| format!("({s})")),
        "via.vec3" | "via.Float3" => floats(3).map(|s| format!("({s})")),
        "via.vec4" | "via.Float4" => floats(4).map(|s| format!("({s})")),
        "via.Quaternion" => floats(4).map(|s| format!("quat({s})")),
        "via.Range" => floats(2).map(|s| format!("range[{s}]")),
        "via.Color" if b.len() >= 4 => Some(format!("rgba({}, {}, {}, {})", b[0], b[1], b[2], b[3])),
        "via.RangeI" if b.len() >= 8 => {
            let lo = i32::from_le_bytes(b[0..4].try_into().unwrap());
            let hi = i32::from_le_bytes(b[4..8].try_into().unwrap());
            Some(format!("range[{lo}, {hi}]"))
        }
        "via.AABB" if b.len() >= 24 => {
            let v: Vec<String> = (0..6).map(|i| fmtf(rd_f32(b, i) as f64)).collect();
            Some(format!("min({}, {}, {}) max({}, {}, {})", v[0], v[1], v[2], v[3], v[4], v[5]))
        }
        "via.Guid" | "via.GameObjectRef" | "via.Uri" if b.len() >= 16 => {
            Some(format!("{{{}}}", guid_str(&b[..16])))
        }
        _ => None,
    }
}

fn fmt_value(out: &mut Vec<String>, indent: usize, prefix: &str, ftype: i32, v: &Value, hint: Option<&str>) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Scalar { off, text, .. } => {
            if let Some(name) = enum_label(hint, text) {
                out.push(format!("{pad}{prefix} {} {name}({text})  @{off:#x}", hint.unwrap()));
            } else {
                out.push(format!("{pad}{prefix} {}={text}  @{off:#x}", type_name(ftype)));
            }
        }
        Value::Str { off, s } => {
            out.push(format!("{pad}{prefix} String={s:?}  @{off:#x}"));
        }
        Value::StructBytes { off, bytes } => {
            if let Some(decoded) = hint.and_then(|h| decode_struct(h, bytes)) {
                out.push(format!("{pad}{prefix} {} {decoded}  @{off:#x}", hint.unwrap()));
            } else {
                let hex: String = bytes.iter().take(16).map(|b| format!("{b:02x}")).collect();
                let ty = hint.map(|h| format!("{h} ")).unwrap_or_default();
                let extra = struct_floats(bytes)
                    .map(|f| format!(" floats=[{f}]"))
                    .unwrap_or_default();
                out.push(format!(
                    "{pad}{prefix} {ty}Struct[{}]={hex}{}  @{off:#x}",
                    bytes.len(),
                    extra
                ));
            }
        }
        Value::Array { member_type, items } => {
            let elem = hint.unwrap_or("");
            out.push(format!(
                "{pad}{prefix} Array<{}>[{}]",
                if elem.is_empty() { type_name(*member_type).to_string() } else { elem.to_string() },
                items.len()
            ));
            for (i, it) in items.iter().enumerate() {
                fmt_value(out, indent + 1, &format!("[{i}]"), *member_type, it, hint);
            }
        }
        Value::Class(c) => {
            out.push(format!("{pad}{prefix} {} ({} fields)", name_for(c.hash), c.fields.len()));
            fmt_class(out, indent + 1, c);
        }
    }
}

fn fmt_class(out: &mut Vec<String>, indent: usize, c: &Class) {
    for f in &c.fields {
        let hint = crate::schema::field_type(c.hash, f.hash);
        fmt_value(out, indent, &name_for(f.hash), f.ftype, &f.value, hint);
    }
    if let Some(t) = &c.truncated {
        out.push(format!("{}-- truncated: {t}", "  ".repeat(indent)));
    }
}

fn flat_value(out: &mut Vec<(String, String, usize)>, path: &str, v: &Value) {
    match v {
        Value::Scalar { off, text, .. } => out.push((path.to_string(), text.clone(), *off)),
        Value::Str { off, s } => out.push((path.to_string(), format!("{s:?}"), *off)),
        Value::StructBytes { off, bytes } => {
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            out.push((path.to_string(), format!("struct:{hex}"), *off));
        }
        Value::Array { items, .. } => {
            out.push((format!("{path}.len"), items.len().to_string(), 0));
            for (i, it) in items.iter().enumerate() {
                flat_value(out, &format!("{path}[{i}]"), it);
            }
        }
        Value::Class(c) => flat_class(out, path, c),
    }
}

fn flat_class(out: &mut Vec<(String, String, usize)>, path: &str, c: &Class) {
    for f in &c.fields {
        let p = format!("{path}.{}", name_for(f.hash));
        flat_value(out, &p, &f.value);
    }
}

pub fn flatten(roots: &[Root]) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        if let Ok(class) = &root.class {
            let p = format!("R{i}#{:08x}", root.native_hash);
            flat_class(&mut out, &p, class);
        }
    }
    out
}

pub fn format(_d: &[u8], roots: &[Root]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        match &root.class {
            Ok(class) => {
                let tag = if class.truncated.is_some() { " [PARTIAL]" } else { "" };
                out.push(format!(
                    "ROOT[{i}] @{:#x} native#{:08x} -> {} ({} fields){tag}",
                    root.offset,
                    root.native_hash,
                    name_for(class.hash),
                    class.fields.len()
                ));
                fmt_class(&mut out, 1, class);
            }
            Err(e) => {
                out.push(format!(
                    "ROOT[{i}] @{:#x} native#{:08x} -- PARSE FAILED: {e}",
                    root.offset, root.native_hash
                ));
            }
        }
    }
    out
}

pub struct Root {
    pub offset: usize,
    pub native_hash: u32,
    pub class: Result<Class, String>,
    pub region_end: usize,
}

const ROOT_SIG: [u8; 8] = [0x01, 0x00, 0x00, 0x00, 0x3c, 0x77, 0x37, 0xe1];

fn root_offsets(d: &[u8]) -> Vec<usize> {
    let mut offs = Vec::new();
    let mut i = 4;
    while i + 8 <= d.len() {
        if d[i..i + 8] == ROOT_SIG {
            offs.push(i - 4);
        }
        i += 1;
    }
    offs
}

#[derive(Clone)]
pub struct EditTarget {
    pub path: String,
    pub off: usize,
    pub ftype: i32,
    pub width: u8,
    pub text: String,
    pub hint: Option<String>,
}

pub fn struct_editable(type_name: &str) -> bool {
    matches!(
        type_name,
        "via.Position"
            | "via.vec2"
            | "via.vec3"
            | "via.vec4"
            | "via.Float2"
            | "via.Float3"
            | "via.Float4"
            | "via.Quaternion"
            | "via.Range"
            | "via.RangeI"
            | "via.Color"
            | "via.Size"
    )
}

fn collect_targets(out: &mut Vec<EditTarget>, path: &str, v: &Value, hint: Option<&str>) {
    match v {
        Value::Scalar {
            off,
            ftype,
            width,
            text,
        } => out.push(EditTarget {
            path: path.to_string(),
            off: *off,
            ftype: *ftype,
            width: *width,
            text: text.clone(),
            hint: hint.map(|s| s.to_string()),
        }),
        Value::StructBytes { off, bytes } => {
            if let Some(h) = hint {
                if struct_editable(h) {
                    if let Some(decoded) = decode_struct(h, bytes) {
                        out.push(EditTarget {
                            path: path.to_string(),
                            off: *off,
                            ftype: 0x10,
                            width: bytes.len() as u8,
                            text: decoded,
                            hint: Some(h.to_string()),
                        });
                    }
                }
            }
        }
        Value::Array { items, .. } => {
            for (i, it) in items.iter().enumerate() {
                collect_targets(out, &format!("{path}[{i}]"), it, hint);
            }
        }
        Value::Class(c) => collect_class(out, path, c),
        _ => {}
    }
}

fn collect_class(out: &mut Vec<EditTarget>, path: &str, c: &Class) {
    for f in &c.fields {
        let hint = crate::schema::field_type(c.hash, f.hash);
        collect_targets(out, &format!("{path}.{}", name_for(f.hash)), &f.value, hint);
    }
}

pub fn edit_targets(roots: &[Root]) -> Vec<EditTarget> {
    let mut out = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        if let Ok(class) = &root.class {
            let p = format!("R{i}#{:08x}", root.native_hash);
            collect_class(&mut out, &p, class);
        }
    }
    out
}

pub fn encode_scalar(ftype: i32, width: u8, text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    let w = width as usize;
    let bad = |e: &str| format!("'{text}' is not a valid {}: {e}", type_name(ftype));
    let bytes: Vec<u8> = match ftype {
        2 => {
            let b = match text {
                "true" | "1" | "True" => 1u8,
                "false" | "0" | "False" => 0u8,
                _ => return Err(bad("expected true/false")),
            };
            vec![b]
        }
        0xb => {
            let f: f32 = text.parse().map_err(|_| bad("float"))?;
            f.to_le_bytes().to_vec()
        }
        0xc => {
            let f: f64 = text.parse().map_err(|_| bad("float"))?;
            f.to_le_bytes().to_vec()
        }
        3 | 5 | 7 | 9 | 1 => {
            let n: i64 = text.parse().map_err(|_| bad("integer"))?;
            n.to_le_bytes()[..w].to_vec()
        }
        4 | 6 | 8 | 0xa | 0xd | 0xe => {
            let n: u64 = if let Some(h) = text.strip_prefix("0x") {
                u64::from_str_radix(h, 16).map_err(|_| bad("hex integer"))?
            } else {
                text.parse().map_err(|_| bad("integer"))?
            };
            n.to_le_bytes()[..w].to_vec()
        }
        _ => return Err(format!("type {} is not editable in place", type_name(ftype))),
    };
    if bytes.len() != w {
        return Err(format!(
            "encoded width {} does not match field width {w}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

pub fn set_scalar(payload: &mut [u8], off: usize, ftype: i32, width: u8, text: &str) -> Result<(), String> {
    let bytes = encode_scalar(ftype, width, text)?;
    if off + bytes.len() > payload.len() {
        return Err(format!("offset {off:#x} out of bounds"));
    }
    payload[off..off + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

fn parse_numbers(text: &str) -> Vec<String> {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphabetic() || "()[]{}".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn set_struct(payload: &mut [u8], off: usize, width: u8, type_name: &str, text: &str) -> Result<(), String> {
    let nums = parse_numbers(text);
    let need = |n: usize| -> Result<(), String> {
        if nums.len() == n {
            Ok(())
        } else {
            Err(format!("{type_name} expects {n} values, got {}", nums.len()))
        }
    };
    let end = off + width as usize;
    if end > payload.len() {
        return Err(format!("offset {off:#x} out of bounds"));
    }
    let put_f32 = |payload: &mut [u8], i: usize, s: &str| -> Result<(), String> {
        let v: f32 = s.parse().map_err(|_| format!("'{s}' is not a float"))?;
        payload[off + i * 4..off + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        Ok(())
    };
    match type_name {
        "via.Position" => {
            need(3)?;
            for (i, s) in nums.iter().enumerate() {
                let v: f64 = s.parse().map_err(|_| format!("'{s}' is not a float"))?;
                payload[off + i * 8..off + i * 8 + 8].copy_from_slice(&v.to_le_bytes());
            }
        }
        "via.vec2" | "via.Float2" | "via.Size" => {
            need(2)?;
            for (i, s) in nums.iter().enumerate() {
                put_f32(payload, i, s)?;
            }
        }
        "via.vec3" | "via.Float3" => {
            need(3)?;
            for (i, s) in nums.iter().enumerate() {
                put_f32(payload, i, s)?;
            }
        }
        "via.vec4" | "via.Float4" | "via.Quaternion" => {
            need(4)?;
            for (i, s) in nums.iter().enumerate() {
                put_f32(payload, i, s)?;
            }
        }
        "via.Range" => {
            need(2)?;
            for (i, s) in nums.iter().enumerate() {
                put_f32(payload, i, s)?;
            }
        }
        "via.RangeI" => {
            need(2)?;
            for (i, s) in nums.iter().enumerate() {
                let v: i32 = s.parse().map_err(|_| format!("'{s}' is not an int"))?;
                payload[off + i * 4..off + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        "via.Color" => {
            need(4)?;
            for (i, s) in nums.iter().enumerate() {
                let v: u8 = s.parse().map_err(|_| format!("'{s}' is not 0..255"))?;
                payload[off + i] = v;
            }
        }
        _ => return Err(format!("{type_name} is not editable")),
    }
    Ok(())
}

pub fn set_target(payload: &mut [u8], t: &EditTarget, text: &str) -> Result<(), String> {
    if t.ftype == 0x10 {
        let ty = t.hint.as_deref().ok_or("struct target has no type")?;
        set_struct(payload, t.off, t.width, ty, text)
    } else {
        set_scalar(payload, t.off, t.ftype, t.width, text)
    }
}

pub fn parse(d: &[u8]) -> Vec<Root> {
    crate::schema::reset_resyncs();
    let mut offs = root_offsets(d);
    if offs.first() != Some(&0) {
        offs.insert(0, 0);
    }
    let mut bounds = offs.clone();
    bounds.push(d.len());

    let mut roots = Vec::new();
    for k in 0..offs.len() {
        let start = offs[k];
        let mut r = Reader::new(d);
        r.pos = start;
        let native_hash = r.u32().unwrap_or(0);
        let class = read_class(&mut r);
        roots.push(Root {
            offset: start,
            native_hash,
            class,
            region_end: bounds[k + 1],
        });
    }
    roots
}
