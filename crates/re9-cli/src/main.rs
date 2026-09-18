use clap::{Parser, Subcommand};
use re9_core::dsss;
use re9_core::mandarin::crack_steamid::crack_steamid;
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
