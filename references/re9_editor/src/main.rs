mod bignum;
mod dsss;
mod edit;
mod elgamal;
mod mandarin;
mod names;
mod rsz;
mod splitmix;

use clap::{Parser, Subcommand};
use edit::ValType;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "re9_editor", about = "Decrypt/edit/encrypt Resident Evil Requiem (DSSS) saves")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Crack {
        file: PathBuf,
    },
    Decrypt {
        file: PathBuf,
        out: PathBuf,
        #[arg(long)]
        steamid: Option<u64>,
    },
    Encrypt {
        plain: PathBuf,
        out: PathBuf,
        #[arg(long)]
        steamid: u64,
    },
    Roundtrip {
        file: PathBuf,
        #[arg(long)]
        steamid: Option<u64>,
    },
    Strings {
        dec: PathBuf,
        #[arg(long, default_value_t = 3)]
        min: usize,
        #[arg(long)]
        grep: Option<String>,
    },
    Show {
        dec: PathBuf,
        offset: String,
        #[arg(default_value_t = 128)]
        len: usize,
    },
    Get {
        dec: PathBuf,
        offset: String,
        ty: String,
    },
    Set {
        dec: PathBuf,
        offset: String,
        ty: String,
        value: String,
    },
    Find {
        dec: PathBuf,
        value: u32,
    },
    Dump {
        dec: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        grep: Option<String>,
    },
    Diff {
        a: PathBuf,
        b: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    Hash {
        name: String,
    },
}

fn resolve_steamid(file: &[u8], given: Option<u64>) -> Result<u64, String> {
    match given {
        Some(id) => Ok(id),
        None => {
            eprintln!("no --steamid given, brute forcing (account id space, up to ~4.3B)...");
            dsss::crack(file).ok_or_else(|| "brute force failed to find a SteamID".to_string())
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Crack { file } => {
            let data = std::fs::read(&file).map_err(|e| e.to_string())?;
            let header = dsss::parse_header(&data)?;
            println!("version: {} flags: {:#x}", header.version, header.flags);
            println!("file hash valid: {}", dsss::verify_file_hash(&data));
            println!("decrypted length: {}", dsss::decrypted_len(&data));
            match dsss::crack(&data) {
                Some(id) => println!("SteamID64: {id}"),
                None => return Err("no SteamID found".into()),
            }
        }
        Command::Decrypt { file, out, steamid } => {
            let data = std::fs::read(&file).map_err(|e| e.to_string())?;
            let id = resolve_steamid(&data, steamid)?;
            let plain = dsss::decrypt(&data, id)?;
            std::fs::write(&out, &plain).map_err(|e| e.to_string())?;
            println!("decrypted {} bytes with SteamID {id} -> {}", plain.len(), out.display());
        }
        Command::Encrypt { plain, out, steamid } => {
            let data = std::fs::read(&plain).map_err(|e| e.to_string())?;
            let file = dsss::build(&data, steamid);
            std::fs::write(&out, &file).map_err(|e| e.to_string())?;
            println!("encrypted {} bytes -> {} ({} bytes)", data.len(), out.display(), file.len());
        }
        Command::Roundtrip { file, steamid } => {
            let data = std::fs::read(&file).map_err(|e| e.to_string())?;
            let id = resolve_steamid(&data, steamid)?;
            let plain = dsss::decrypt(&data, id)?;
            println!("decrypt ok: {} bytes (checksums passed)", plain.len());
            let rebuilt = dsss::build(&plain, id);
            let plain2 = dsss::decrypt(&rebuilt, id)?;
            if plain == plain2 {
                println!("roundtrip ok: re-encrypt -> decrypt reproduces identical payload");
            } else {
                return Err("roundtrip mismatch: payload changed".into());
            }
            println!("rebuilt file hash valid: {}", dsss::verify_file_hash(&rebuilt));
            let byte_equal = rebuilt == data;
            println!("rebuilt == original file: {byte_equal} (len {} vs {})", rebuilt.len(), data.len());
        }
        Command::Strings { dec, min, grep } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let mut count = 0;
            for (off, s) in edit::utf16_strings(&data, min) {
                if let Some(g) = &grep {
                    if !s.contains(g.as_str()) {
                        continue;
                    }
                }
                println!("{off:#08x}  {s}");
                count += 1;
            }
            eprintln!("{count} strings");
        }
        Command::Show { dec, offset, len } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let off = edit::parse_offset(&offset)?;
            print!("{}", edit::hexdump(&data, off, len));
        }
        Command::Get { dec, offset, ty } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let off = edit::parse_offset(&offset)?;
            let ty = ValType::parse(&ty)?;
            if off + ty.size() > data.len() {
                return Err("offset out of bounds".into());
            }
            println!("{}", ty.read(&data[off..]));
        }
        Command::Set { dec, offset, ty, value } => {
            let mut data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let off = edit::parse_offset(&offset)?;
            let ty = ValType::parse(&ty)?;
            if off + ty.size() > data.len() {
                return Err("offset out of bounds".into());
            }
            let old = ty.read(&data[off..]);
            let bytes = ty.encode(&value)?;
            data[off..off + bytes.len()].copy_from_slice(&bytes);
            std::fs::write(&dec, &data).map_err(|e| e.to_string())?;
            println!("{off:#08x}: {old} -> {value}");
        }
        Command::Find { dec, value } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let hits = edit::find_u32(&data, value);
            for off in &hits {
                println!("{off:#08x}");
            }
            eprintln!("{} matches for u32 {value}", hits.len());
        }
        Command::Dump { dec, out, grep } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let ok = roots.iter().filter(|r| r.class.is_ok()).count();
            let mut lines = rsz::format(&data, &roots);
            if let Some(g) = &grep {
                lines.retain(|l| l.contains(g.as_str()));
            }
            let body = lines.join("\n");
            match out {
                Some(path) => {
                    std::fs::write(&path, format!("{body}\n")).map_err(|e| e.to_string())?;
                    eprintln!("{} lines -> {}", lines.len(), path.display());
                }
                None => println!("{body}"),
            }
            eprintln!("{}/{} roots parsed structurally (rest fell back to string scan), {} names loaded", ok, roots.len(), names::count());
        }
        Command::Diff { a, b, out } => {
            let da = std::fs::read(&a).map_err(|e| e.to_string())?;
            let db = std::fs::read(&b).map_err(|e| e.to_string())?;
            let fa = rsz::flatten(&rsz::parse(&da));
            let fb = rsz::flatten(&rsz::parse(&db));
            use std::collections::BTreeMap;
            let ma: BTreeMap<_, _> = fa.iter().map(|(p, v, o)| (p.clone(), (v.clone(), *o))).collect();
            let mb: BTreeMap<_, _> = fb.iter().map(|(p, v, o)| (p.clone(), (v.clone(), *o))).collect();
            let mut lines = Vec::new();
            let mut keys: Vec<&String> = ma.keys().chain(mb.keys()).collect();
            keys.sort();
            keys.dedup();
            let (mut changed, mut only_a, mut only_b) = (0, 0, 0);
            for k in keys {
                match (ma.get(k), mb.get(k)) {
                    (Some((va, oa)), Some((vb, _))) => {
                        if va != vb {
                            lines.push(format!("~ {k}\n    A: {va}  @{oa:#x}\n    B: {vb}"));
                            changed += 1;
                        }
                    }
                    (Some((va, oa)), None) => {
                        lines.push(format!("- {k} = {va}  @{oa:#x} (only A)"));
                        only_a += 1;
                    }
                    (None, Some((vb, _))) => {
                        lines.push(format!("+ {k} = {vb} (only B)"));
                        only_b += 1;
                    }
                    (None, None) => {}
                }
            }
            let body = lines.join("\n");
            match out {
                Some(path) => {
                    std::fs::write(&path, format!("{body}\n")).map_err(|e| e.to_string())?;
                    eprintln!("{} diffs -> {}", lines.len(), path.display());
                }
                None => println!("{body}"),
            }
            eprintln!("{changed} changed, {only_a} only in A, {only_b} only in B");
        }
        Command::Hash { name } => {
            println!("{:08x}  {name}", names::murmur3(&name));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
