use clap::{Parser, Subcommand};
use re9_core::dsss;
use re9_core::mandarin::crack_steamid::crack_steamid;
use re9_core::{inventory, murmur3, rsz};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "re9",
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
    Crack {
        file: PathBuf,
    },
    /// Decrypt a DSSS save into its raw RSZ payload.
    Decrypt {
        file: PathBuf,
        /// Output file (defaults to `<file>_new.<ext>`)
        #[arg(long)]
        out: Option<PathBuf>,
        /// SteamID64 to decrypt with; if omitted, it is brute-forced.
        #[arg(long)]
        steamid: Option<u64>,
    },
    /// Re-encrypt a raw RSZ payload into a loadable DSSS save.
    Encrypt {
        file: PathBuf,
        /// Output file (defaults to `<file>_new.<ext>`)
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        steamid: u64,
    },
    /// Decrypt, re-encrypt, decrypt again and verify the payload is stable.
    Roundtrip {
        file: PathBuf,
        #[arg(long)]
        steamid: Option<u64>,
    },
    /// Print the full self-describing RSZ tree of a decrypted payload.
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
    /// List every field (including strings/structs/array lengths) by its
    /// flattened RSZ path - useful to explore an unfamiliar structure.
    Paths {
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
    /// Compute the murmur3 hash of a field/class name (to guess field hashes).
    Hash { name: String },
    /// List every inventory slot (container + item + quantity) found in a
    /// decrypted payload.
    Inventory {
        dec: PathBuf,
        #[arg(long)]
        container: Option<String>,
        /// Filter by owner (e.g. "User00", "User01").
        #[arg(long)]
        owner: Option<String>,
    },
    /// Set an inventory slot's quantity in place, by container name + item
    /// index (see `inventory` to list them).
    SetQuantity {
        dec: PathBuf,
        /// Container name, e.g. "Hand" or "ItemBox".
        container: String,
        /// Which occurrence of that container name (0-based; there can be
        /// several containers sharing the same name).
        container_index: usize,
        /// Index of the item within that container's item array.
        item_index: usize,
        quantity: i32,
    },
    /// Set a weapon's loaded magazine ammo in place
    /// (`_LoadingItems[loaded_index]._AmountSaveData._Stock`).
    SetLoadedQuantity {
        dec: PathBuf,
        container: String,
        container_index: usize,
        item_index: usize,
        /// Index within the item's `_LoadingItems` array (see `inventory`).
        loaded_index: usize,
        quantity: i32,
    },
    /// Set a weapon's chamber ammo in place (`_LoadingItems[loaded_index]._ChamberStock`).
    SetChamberQuantity {
        dec: PathBuf,
        container: String,
        container_index: usize,
        item_index: usize,
        loaded_index: usize,
        quantity: i32,
    },
}

/// `foo/bar.bin` -> `foo/bar_new.bin`
fn default_out_path(input: &Path) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = input.extension().and_then(|s| s.to_str());
    let file_name = match ext {
        Some(ext) => format!("{stem}_new.{ext}"),
        None => format!("{stem}_new"),
    };
    input.with_file_name(file_name)
}

fn resolve_steamid(file: &[u8], given: Option<u64>) -> u64 {
    match given {
        Some(id) => id,
        None => {
            eprintln!("no --steamid given, brute forcing the account-id space (~4.3B)...");
            let decrypted_len = dsss::decrypted_len(file);
            let payload = dsss::payload(file);
            let first_bytes: [u8; 8] = payload[0..8].try_into().expect("payload too small");
            crack_steamid(first_bytes, decrypted_len).unwrap_or_else(|| {
                eprintln!("error: could not recover a SteamID by brute force");
                std::process::exit(1);
            })
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Crack { file } => {
            let data = std::fs::read(&file).map_err(|e| e.to_string())?;
            let id = resolve_steamid(&data, None);
            println!("SteamID64: {id}");
        }
        Command::Decrypt { file, out, steamid } => {
            let data = std::fs::read(&file).map_err(|e| e.to_string())?;
            let id = resolve_steamid(&data, steamid);
            let plain = dsss::decrypt(&data, id);
            let out = out.unwrap_or_else(|| default_out_path(&file));
            std::fs::write(&out, &plain).map_err(|e| e.to_string())?;
            println!(
                "decrypted {} bytes with SteamID {id} -> {}",
                plain.len(),
                out.display()
            );
        }
        Command::Encrypt { file, out, steamid } => {
            let plain = std::fs::read(&file).map_err(|e| e.to_string())?;
            let built = dsss::build(&plain, steamid);
            let out = out.unwrap_or_else(|| default_out_path(&file));
            std::fs::write(&out, &built).map_err(|e| e.to_string())?;
            println!(
                "encrypted {} bytes -> {} ({} bytes)",
                plain.len(),
                out.display(),
                built.len()
            );
        }
        Command::Roundtrip { file, steamid } => {
            let data = std::fs::read(&file).map_err(|e| e.to_string())?;
            let id = resolve_steamid(&data, steamid);
            let plain = dsss::decrypt(&data, id);
            println!("decrypt ok: {} bytes", plain.len());
            let rebuilt = dsss::build(&plain, id);
            let plain2 = dsss::decrypt(&rebuilt, id);
            if plain != plain2 {
                return Err("roundtrip mismatch: payload changed".to_string());
            }
            println!("roundtrip ok: re-encrypt -> decrypt reproduces identical payload");
            println!("rebuilt file hash valid: {}", dsss::verify_file_hash(&rebuilt));
            println!(
                "rebuilt == original file: {} (len {} vs {})",
                rebuilt == data,
                rebuilt.len(),
                data.len()
            );
        }
        Command::Dump { dec, out, grep } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
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
                    std::fs::write(&path, format!("{body}\n")).map_err(|e| e.to_string())?;
                    eprintln!("{} lines -> {}", lines.len(), path.display());
                }
                None => println!("{body}"),
            }
            eprintln!(
                "{full}/{} roots fully parsed, {partial} partial (truncated at a still-unmodeled struct), {} names, {} schema classes, {} resyncs",
                roots.len(),
                re9_core::names::count(),
                re9_core::schema::count(),
                re9_core::schema::resyncs()
            );
        }
        Command::Targets { dec, grep } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
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
                println!("@{:#08x}  {ty} {} = {}", t.off, t.path, t.text);
                count += 1;
            }
            eprintln!("{count} editable scalar fields");
        }
        Command::Paths { dec, grep } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let flat = rsz::flatten(&roots);
            let mut count = 0;
            for (path, value, off) in &flat {
                if let Some(g) = &grep {
                    if !path.contains(g.as_str()) {
                        continue;
                    }
                }
                println!("@{off:#08x}  {path} = {value}");
                count += 1;
            }
            eprintln!("{count} fields");
        }
        Command::SetPath { dec, path, value } => {
            let mut data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let targets = rsz::edit_targets(&roots);
            let exact: Vec<_> = targets.iter().filter(|t| t.path == path).collect();
            let chosen = if let [t] = exact.as_slice() {
                *t
            } else {
                let subs: Vec<_> = targets.iter().filter(|t| t.path.contains(&path)).collect();
                match subs.as_slice() {
                    [t] => *t,
                    [] => return Err(format!("no field matches path '{path}'")),
                    many => {
                        let preview: Vec<_> = many.iter().take(8).map(|t| t.path.as_str()).collect();
                        return Err(format!(
                            "'{path}' is ambiguous ({} matches): {}{}",
                            many.len(),
                            preview.join(", "),
                            if many.len() > 8 { ", ..." } else { "" }
                        ));
                    }
                }
            };
            let old = chosen.text.clone();
            rsz::set_target(&mut data, chosen, &value)?;
            std::fs::write(&dec, &data).map_err(|e| e.to_string())?;
            println!("@{:#08x}  {} {} -> {}", chosen.off, chosen.path, old, value);
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
            println!("{:08x}  {name}", murmur3::name_hash(&name));
        }
        Command::Inventory { dec, container, owner } => {
            let data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let slots = inventory::list_inventory(&roots);
            let mut count = 0;
            for s in &slots {
                if let Some(c) = &container {
                    if &s.container != c {
                        continue;
                    }
                }
                if let Some(o) = &owner {
                    if &s.owner != o {
                        continue;
                    }
                }
                let item = s.item_id.as_deref().unwrap_or("???");
                let name = s.item_name.map(|n| format!(" \"{n}\"")).unwrap_or_default();
                println!(
                    "{}/{}#{} [{}]  qty={}  item={item}{name} (item_id_hash={:#010x})  @{:#x}",
                    s.owner, s.container, s.container_index, s.item_index, s.quantity, s.item_id_hash, s.quantity_offset
                );
                for l in &s.loaded {
                    let loaded_item = l.item_id.as_deref().unwrap_or("???");
                    let loaded_name = l.item_name.map(|n| format!(" \"{n}\"")).unwrap_or_default();
                    println!(
                        "    loaded[{}]  item={loaded_item}{loaded_name} (item_id_hash={:#010x})  stock={} @{:#x}  chamber={} @{:#x}",
                        l.loaded_index, l.item_id_hash, l.stock, l.stock_offset, l.chamber_stock, l.chamber_stock_offset
                    );
                }
                count += 1;
            }
            eprintln!("{count} inventory slots");
        }
        Command::SetQuantity {
            dec,
            container,
            container_index,
            item_index,
            quantity,
        } => {
            let mut data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let slots = inventory::list_inventory(&roots);
            let slot = slots
                .iter()
                .find(|s| {
                    s.container == container
                        && s.container_index == container_index
                        && s.item_index == item_index
                })
                .ok_or_else(|| {
                    format!("no such slot: {container}#{container_index} [{item_index}]")
                })?;
            let old = slot.quantity;
            inventory::set_quantity(&mut data, slot, quantity)?;
            std::fs::write(&dec, &data).map_err(|e| e.to_string())?;
            println!(
                "{}#{} [{}]  qty: {old} -> {quantity}  @{:#x}",
                slot.container, slot.container_index, slot.item_index, slot.quantity_offset
            );
        }
        Command::SetLoadedQuantity {
            dec,
            container,
            container_index,
            item_index,
            loaded_index,
            quantity,
        } => {
            let mut data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let slots = inventory::list_inventory(&roots);
            let slot = slots
                .iter()
                .find(|s| {
                    s.container == container
                        && s.container_index == container_index
                        && s.item_index == item_index
                })
                .ok_or_else(|| {
                    format!("no such slot: {container}#{container_index} [{item_index}]")
                })?;
            let loaded = slot
                .loaded
                .iter()
                .find(|l| l.loaded_index == loaded_index)
                .ok_or_else(|| {
                    format!(
                        "no such loaded-ammo entry: {container}#{container_index} [{item_index}].loaded[{loaded_index}]"
                    )
                })?;
            let old = loaded.stock;
            inventory::set_loaded_stock(&mut data, loaded, quantity)?;
            std::fs::write(&dec, &data).map_err(|e| e.to_string())?;
            println!(
                "{container}#{container_index} [{item_index}].loaded[{loaded_index}]  stock: {old} -> {quantity}  @{:#x}",
                loaded.stock_offset
            );
        }
        Command::SetChamberQuantity {
            dec,
            container,
            container_index,
            item_index,
            loaded_index,
            quantity,
        } => {
            let mut data = std::fs::read(&dec).map_err(|e| e.to_string())?;
            let roots = rsz::parse(&data);
            let slots = inventory::list_inventory(&roots);
            let slot = slots
                .iter()
                .find(|s| {
                    s.container == container
                        && s.container_index == container_index
                        && s.item_index == item_index
                })
                .ok_or_else(|| {
                    format!("no such slot: {container}#{container_index} [{item_index}]")
                })?;
            let loaded = slot
                .loaded
                .iter()
                .find(|l| l.loaded_index == loaded_index)
                .ok_or_else(|| {
                    format!(
                        "no such loaded-ammo entry: {container}#{container_index} [{item_index}].loaded[{loaded_index}]"
                    )
                })?;
            let old = loaded.chamber_stock;
            inventory::set_chamber_stock(&mut data, loaded, quantity)?;
            std::fs::write(&dec, &data).map_err(|e| e.to_string())?;
            println!(
                "{container}#{container_index} [{item_index}].loaded[{loaded_index}]  chamber: {old} -> {quantity}  @{:#x}",
                loaded.chamber_stock_offset
            );
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
