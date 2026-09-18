use clap::{Parser, Subcommand};
use re9::edit::ValType;
use re9::error::{Error, Result};
use re9::{dsss, edit, names, rsz};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

mod term {
    use std::sync::OnceLock;

    fn enabled() -> bool {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| {
            std::env::var_os("NO_COLOR").is_none()
                && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
        })
    }

    fn wrap(code: &str, s: &str) -> String {
        if enabled() {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    pub fn green(s: &str) -> String {
        wrap("1;32", s)
    }
    pub fn red(s: &str) -> String {
        wrap("1;31", s)
    }
    pub fn yellow(s: &str) -> String {
        wrap("1;33", s)
    }
    pub fn cyan(s: &str) -> String {
        wrap("36", s)
    }
    pub fn dim(s: &str) -> String {
        wrap("2", s)
    }
    pub fn bold(s: &str) -> String {
        wrap("1", s)
    }
}

#[derive(Parser)]
#[command(
    name = "re9_editor",
    version,
    about = "Decrypt/edit/encrypt Resident Evil Requiem (DSSS) saves"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Recover the SteamID a save was encrypted for (brute force, ~4.3B space).
    Crack { file: PathBuf },
    /// Decrypt a DSSS save to its raw RSZ payload.
    Decrypt {
        file: PathBuf,
        out: PathBuf,
        #[arg(long)]
        steamid: Option<u64>,
    },
    /// Re-encrypt a raw payload into a loadable DSSS save.
    Encrypt {
        plain: PathBuf,
        out: PathBuf,
        #[arg(long)]
        steamid: u64,
    },
    /// Decrypt, re-encrypt, decrypt again and verify the payload is stable.
    Roundtrip {
        file: PathBuf,
        #[arg(long)]
        steamid: Option<u64>,
    },
    /// List UTF-16 strings in a decrypted payload.
    Strings {
        dec: PathBuf,
        #[arg(long, default_value_t = 3)]
        min: usize,
        #[arg(long)]
        grep: Option<String>,
    },
    /// Hex dump a region of a decrypted payload.
    Show {
        dec: PathBuf,
        offset: String,
        #[arg(default_value_t = 128)]
        len: usize,
    },
    /// Read a typed value at a raw byte offset.
    Get {
        dec: PathBuf,
        offset: String,
        ty: String,
    },
    /// Write a typed value at a raw byte offset.
    Set {
        dec: PathBuf,
        offset: String,
        ty: String,
        value: String,
    },
    /// Find every offset where a u32 little-endian value occurs.
    Find { dec: PathBuf, value: u32 },
    /// Print the full self-describing RSZ tree.
    Dump {
        dec: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        grep: Option<String>,
    },
    /// List every editable scalar field by its RSZ path.
    Targets {
        dec: PathBuf,
        #[arg(long)]
        grep: Option<String>,
    },
    /// Edit a scalar field in place by its RSZ path (exact or unique substring).
    SetPath {
        dec: PathBuf,
        path: String,
        value: String,
    },
    /// Compare two decrypted payloads field-by-field along RSZ paths.
    Diff {
        a: PathBuf,
        b: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Compute the murmur3 (seed 0xffffffff) hash of a field name.
    Hash { name: String },
}

fn backup(path: &std::path::Path) {
    if path.exists() {
        let mut b = path.as_os_str().to_os_string();
        b.push(".bak");
        if std::fs::copy(path, std::path::PathBuf::from(&b)).is_ok() {
            eprintln!("{} {}", term::dim("backup ->"), b.to_string_lossy());
        }
    }
}

fn resolve_steamid(file: &[u8], given: Option<u64>) -> Result<u64> {
    match given {
        Some(id) => Ok(id),
        None => {
            eprintln!(
                "{}",
                term::yellow("no --steamid given, brute forcing the account-id space (~4.3B)...")
            );
            let total = dsss::steamid_count();
            let last = AtomicU64::new(0);
            let found = dsss::crack_with(file, |done| {
                let prev = last.swap(done, Ordering::Relaxed);
                if done / 50_000_000 != prev / 50_000_000 || done >= total {
                    let pct = done as f64 / total as f64 * 100.0;
                    eprint!("\r  {} {pct:5.1}%  ({done} / {total})", term::dim("cracking"));
                    let _ = std::io::stderr().flush();
                }
            });
            eprintln!();
            found.ok_or(Error::CrackFailed)
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Crack { file } => {
            let data = std::fs::read(&file)?;
            let header = dsss::parse_header(&data)?;
            println!(
                "version: {} flags: {}",
                header.version,
                term::cyan(&format!("{:#x}", header.flags))
            );
            let valid = dsss::verify_file_hash(&data);
            println!(
                "file hash valid: {}",
                if valid {
                    term::green("true")
                } else {
                    term::red("false")
                }
            );
            println!("decrypted length: {}", dsss::decrypted_len(&data));
            let id = resolve_steamid(&data, None)?;
            println!("SteamID64: {}", term::green(&id.to_string()));
        }
        Command::Decrypt { file, out, steamid } => {
            let data = std::fs::read(&file)?;
            let id = resolve_steamid(&data, steamid)?;
            let plain = dsss::decrypt(&data, id)?;
            std::fs::write(&out, &plain)?;
            println!(
                "{} {} bytes with SteamID {} -> {}",
                term::green("decrypted"),
                plain.len(),
                id,
                out.display()
            );
        }
        Command::Encrypt { plain, out, steamid } => {
            let data = std::fs::read(&plain)?;
            let file = dsss::build(&data, steamid);
            backup(&out);
            std::fs::write(&out, &file)?;
            println!(
                "{} {} bytes -> {} ({} bytes)",
                term::green("encrypted"),
                data.len(),
                out.display(),
                file.len()
            );
        }
        Command::Roundtrip { file, steamid } => {
            let data = std::fs::read(&file)?;
            let id = resolve_steamid(&data, steamid)?;
            let plain = dsss::decrypt(&data, id)?;
            println!("decrypt ok: {} bytes (checksums passed)", plain.len());
            let rebuilt = dsss::build(&plain, id);
            let plain2 = dsss::decrypt(&rebuilt, id)?;
            if plain != plain2 {
                return Err(Error::Decrypt("roundtrip mismatch: payload changed".into()));
            }
            println!(
                "{}",
                term::green("roundtrip ok: re-encrypt -> decrypt reproduces identical payload")
            );
            println!("rebuilt file hash valid: {}", dsss::verify_file_hash(&rebuilt));
            let byte_equal = rebuilt == data;
            println!(
                "rebuilt == original file: {byte_equal} (len {} vs {})",
                rebuilt.len(),
                data.len()
            );
        }
        Command::Strings { dec, min, grep } => {
            let data = std::fs::read(&dec)?;
            let mut count = 0;
            for (off, s) in edit::utf16_strings(&data, min) {
                if let Some(g) = &grep {
                    if !s.contains(g.as_str()) {
                        continue;
                    }
                }
                println!("{}  {s}", term::dim(&format!("{off:#08x}")));
                count += 1;
            }
            eprintln!("{count} strings");
        }
        Command::Show { dec, offset, len } => {
            let data = std::fs::read(&dec)?;
            let off = edit::parse_offset(&offset)?;
            print!("{}", edit::hexdump(&data, off, len));
        }
        Command::Get { dec, offset, ty } => {
            let data = std::fs::read(&dec)?;
            let off = edit::parse_offset(&offset)?;
            let ty = ValType::parse(&ty)?;
            if off + ty.size() > data.len() {
                return Err(Error::OutOfBounds { off, len: data.len() });
            }
            println!("{}", ty.read(&data[off..]));
        }
        Command::Set {
            dec,
            offset,
            ty,
            value,
        } => {
            let mut data = std::fs::read(&dec)?;
            let off = edit::parse_offset(&offset)?;
            let ty = ValType::parse(&ty)?;
            if off + ty.size() > data.len() {
                return Err(Error::OutOfBounds { off, len: data.len() });
            }
            let old = ty.read(&data[off..]);
            let bytes = ty.encode(&value).map_err(Error::Edit)?;
            data[off..off + bytes.len()].copy_from_slice(&bytes);
            backup(&dec);
            std::fs::write(&dec, &data)?;
            println!(
                "{off:#08x}: {} -> {}",
                term::dim(&old),
                term::green(&value)
            );
        }
        Command::Find { dec, value } => {
            let data = std::fs::read(&dec)?;
            let hits = edit::find_u32(&data, value);
            for off in &hits {
                println!("{off:#08x}");
            }
            eprintln!("{} matches for u32 {value}", hits.len());
        }
        Command::Dump { dec, out, grep } => {
            let data = std::fs::read(&dec)?;
            let roots = rsz::parse(&data);
            let full = roots
                .iter()
                .filter(|r| r.class.as_ref().map(|c| c.truncated.is_none()).unwrap_or(false))
                .count();
            let partial = roots
                .iter()
                .filter(|r| r.class.as_ref().map(|c| c.truncated.is_some()).unwrap_or(false))
                .count();
            let mut lines = rsz::format(&data, &roots);
            if let Some(g) = &grep {
                lines.retain(|l| l.contains(g.as_str()));
            }
            let body = lines.join("\n");
            match out {
                Some(path) => {
                    std::fs::write(&path, format!("{body}\n"))?;
                    eprintln!("{} lines -> {}", lines.len(), path.display());
                }
                None => println!("{body}"),
            }
            eprintln!(
                "{full}/{} roots fully parsed, {partial} partial (truncated at a still-unmodeled struct), {} names, {} schema classes, {} resyncs",
                roots.len(),
                names::count(),
                re9::schema::count(),
                re9::schema::resyncs()
            );
        }
        Command::Targets { dec, grep } => {
            let data = std::fs::read(&dec)?;
            let roots = rsz::parse(&data);
            let targets = rsz::edit_targets(&roots);
            let mut count = 0;
            for t in &targets {
                if let Some(g) = &grep {
                    if !t.path.contains(g.as_str()) {
                        continue;
                    }
                }
                let ty = t.hint.as_deref().unwrap_or_else(|| rsz::type_name(t.ftype));
                println!(
                    "{}  {} {} = {}",
                    term::dim(&format!("@{:#08x}", t.off)),
                    term::cyan(ty),
                    t.path,
                    term::bold(&t.text)
                );
                count += 1;
            }
            eprintln!("{count} editable scalar fields");
        }
        Command::SetPath { dec, path, value } => {
            let mut data = std::fs::read(&dec)?;
            let roots = rsz::parse(&data);
            let targets = rsz::edit_targets(&roots);
            let exact: Vec<_> = targets.iter().filter(|t| t.path == path).collect();
            let chosen = if let [t] = exact.as_slice() {
                *t
            } else {
                let subs: Vec<_> = targets.iter().filter(|t| t.path.contains(&path)).collect();
                match subs.as_slice() {
                    [t] => *t,
                    [] => return Err(Error::Edit(format!("no field matches path '{path}'"))),
                    many => {
                        let preview: Vec<_> =
                            many.iter().take(8).map(|t| t.path.as_str()).collect();
                        return Err(Error::Edit(format!(
                            "'{path}' is ambiguous ({} matches): {}{}",
                            many.len(),
                            preview.join(", "),
                            if many.len() > 8 { ", ..." } else { "" }
                        )));
                    }
                }
            };
            let old = chosen.text.clone();
            rsz::set_target(&mut data, chosen, &value).map_err(Error::Edit)?;
            backup(&dec);
            std::fs::write(&dec, &data)?;
            println!(
                "{}  {} {} -> {}",
                term::dim(&format!("@{:#08x}", chosen.off)),
                chosen.path,
                term::dim(&old),
                term::green(&value)
            );
        }
        Command::Diff { a, b, out } => {
            let da = std::fs::read(&a)?;
            let db = std::fs::read(&b)?;
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
                            lines.push(format!(
                                "{} {k}\n    A: {va}  @{oa:#x}\n    B: {vb}",
                                term::yellow("~")
                            ));
                            changed += 1;
                        }
                    }
                    (Some((va, oa)), None) => {
                        lines.push(format!("{} {k} = {va}  @{oa:#x} (only A)", term::red("-")));
                        only_a += 1;
                    }
                    (None, Some((vb, _))) => {
                        lines.push(format!("{} {k} = {vb} (only B)", term::green("+")));
                        only_b += 1;
                    }
                    (None, None) => {}
                }
            }
            let body = lines.join("\n");
            match out {
                Some(path) => {
                    std::fs::write(&path, format!("{body}\n"))?;
                    eprintln!("{} diffs -> {}", lines.len(), path.display());
                }
                None => println!("{body}"),
            }
            eprintln!("{changed} changed, {only_a} only in A, {only_b} only in B");
        }
        Command::Hash { name } => {
            println!("{}  {name}", term::cyan(&format!("{:08x}", names::murmur3(&name))));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{} {e}", term::red("error:"));
            ExitCode::FAILURE
        }
    }
}
