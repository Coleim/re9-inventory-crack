use crate::names::name_for;

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

pub enum Value {
    Scalar { off: usize, text: String },
    Str { off: usize, s: String },
    StructBytes { off: usize, bytes: Vec<u8> },
    Array { member_type: i32, items: Vec<Value> },
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
            let _ = size;
            Ok(Value::Scalar { off, text })
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
            0 => {
                if member_type == 0xf {
                    read_value(r, 0xf)?
                } else {
                    let off_pre = r.pos;
                    if member_size != 1 {
                        r.align(member_size as usize);
                    }
                    let off = r.pos;
                    let text = read_scalar_text(r, member_type, member_size)?;
                    let _ = off_pre;
                    Value::Scalar { off, text }
                }
            }
            _ => Value::Class(read_class(r)?),
        };
        items.push(v);
    }
    r.align(4);
    Ok(Value::Array { member_type, items })
}

fn read_class(r: &mut Reader) -> Result<Class, String> {
    let num_fields = r.u32()?;
    let hash = r.u32()?;
    if num_fields > 100_000 {
        return Err(format!("class num_fields too big: {num_fields}"));
    }
    let mut fields = Vec::new();
    for fi in 0..num_fields {
        let fstart = r.pos;
        let fhash = r.u32()?;
        let ftype = r.i32()?;
        let value = read_value(r, ftype)
            .map_err(|e| format!("class#{hash:08x} field {fi}/{num_fields} @{fstart:#x} (hash {fhash:08x} type {ftype}): {e}"))?;
        r.align(4);
        fields.push(Field {
            hash: fhash,
            ftype,
            value,
        });
    }
    Ok(Class { hash, fields })
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

fn fmt_value(out: &mut Vec<String>, indent: usize, prefix: &str, ftype: i32, v: &Value) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Scalar { off, text, .. } => {
            out.push(format!("{pad}{prefix} {}={text}  @{off:#x}", type_name(ftype)));
        }
        Value::Str { off, s } => {
            out.push(format!("{pad}{prefix} String={s:?}  @{off:#x}"));
        }
        Value::StructBytes { off, bytes } => {
            let hex: String = bytes.iter().take(16).map(|b| format!("{b:02x}")).collect();
            let extra = struct_floats(bytes)
                .map(|f| format!(" floats=[{f}]"))
                .unwrap_or_default();
            out.push(format!(
                "{pad}{prefix} Struct[{}]={hex}{}  @{off:#x}",
                bytes.len(),
                extra
            ));
        }
        Value::Array { member_type, items } => {
            out.push(format!(
                "{pad}{prefix} Array<{}>[{}]",
                type_name(*member_type),
                items.len()
            ));
            for (i, it) in items.iter().enumerate() {
                fmt_value(out, indent + 1, &format!("[{i}]"), *member_type, it);
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
        fmt_value(out, indent, &name_for(f.hash), f.ftype, &f.value);
    }
}

fn flat_value(out: &mut Vec<(String, String, usize)>, path: &str, v: &Value) {
    match v {
        Value::Scalar { off, text } => out.push((path.to_string(), text.clone(), *off)),
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

pub fn format(d: &[u8], roots: &[Root]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        match &root.class {
            Ok(class) => {
                out.push(format!(
                    "ROOT[{i}] @{:#x} native#{:08x} -> {} ({} fields)",
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
                out.push(format!(
                    "  loose string scan of region {:#x}..{:#x}:",
                    root.offset, root.region_end
                ));
                out.extend(loose_strings(d, root.offset, root.region_end));
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

pub fn loose_strings(d: &[u8], start: usize, end: usize) -> Vec<String> {
    let mut out = Vec::new();
    let end = end.min(d.len());
    let mut i = start;
    while i + 1 < end {
        if (0x20..0x7f).contains(&d[i]) && d[i + 1] == 0 {
            let s0 = i;
            let mut s = String::new();
            while i + 1 < end && (0x20..0x7f).contains(&d[i]) && d[i + 1] == 0 {
                s.push(d[i] as char);
                i += 2;
            }
            if s.len() >= 3 {
                let after = (i + 3) & !3;
                let mut ctx = Vec::new();
                for k in 0..3 {
                    let o = after + k * 4;
                    if o + 4 <= end {
                        let u = u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                        ctx.push(format!("{u}@{o:#x}"));
                    }
                }
                out.push(format!("  {s0:#08x}  {s:?}  next: {}", ctx.join(" ")));
            }
        } else {
            i += 1;
        }
    }
    out
}

pub fn parse(d: &[u8]) -> Vec<Root> {
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
