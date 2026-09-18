use eframe::egui;
use egui::{Color32, RichText};
use re9::rsz::{self, Class, EditTarget, Root, Value};
use re9::{dsss, edit, names};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};

const ACCENT: Color32 = Color32::from_rgb(0xff, 0x6b, 0x35);
const ACCENT_DIM: Color32 = Color32::from_rgb(0xc4, 0x52, 0x28);
const GOOD: Color32 = Color32::from_rgb(0x6b, 0xcb, 0x77);
const BAD: Color32 = Color32::from_rgb(0xff, 0x5d, 0x5d);
const KEY: Color32 = Color32::from_rgb(0x8a, 0xb4, 0xf8);
const NUM: Color32 = Color32::from_rgb(0xf2, 0xc9, 0x4c);
const STR: Color32 = Color32::from_rgb(0xa3, 0xe6, 0x8c);
const MUTED: Color32 = Color32::from_rgb(0x9a, 0x9a, 0xb0);

type Changed = std::collections::HashSet<usize>;

#[derive(PartialEq, Clone, Copy, Default)]
enum Tab {
    #[default]
    Gameplay,
    Tree,
    Hex,
    Strings,
    Watch,
}

const GAMEPLAY_KEYS: &[&str] = &[
    "SaveCount", "Stock", "Amount", "SerialNumber", "Money", "Gold", "Cash", "Ammo", "Level", "Exp",
    "Health", "Hp", "Point", "Skill", "Unlock", "Progress", "PlayTime", "Difficulty", "Item",
    "Quantity", "Count", "Number",
];

fn is_gameplay(path: &str) -> bool {
    let last = path.rsplit('.').next().unwrap_or(path);
    let last = last.split('#').next().unwrap_or(last);
    GAMEPLAY_KEYS.iter().any(|k| last.contains(k))
}

#[derive(PartialEq, Clone, Copy)]
enum Kind {
    Info,
    Ok,
    Err,
}

struct CrackJob {
    done: Arc<AtomicU64>,
    total: u64,
    rx: Receiver<Option<u64>>,
}

#[derive(Clone)]
struct Selected {
    target: EditTarget,
    buf: String,
}

#[derive(Default)]
struct App {
    raw: Option<Vec<u8>>,
    path: Option<PathBuf>,
    is_dsss: bool,
    version: u32,
    flags: u32,
    hash_valid: bool,
    dec_len: u64,

    steamid_input: String,
    steamid: Option<u64>,

    payload: Option<Vec<u8>>,
    roots: Vec<Root>,
    targets: Vec<EditTarget>,
    ok_roots: usize,

    selected: Option<Selected>,
    search: String,
    hex_goto: String,
    str_filter: String,

    dirty: bool,
    status: String,
    status_kind: Option<Kind>,
    crack: Option<CrackJob>,
    tab: Tab,

    watching: bool,
    watch_mtime: Option<std::time::SystemTime>,
    watch_size: u64,
    watch_pending: Option<(Option<std::time::SystemTime>, u64)>,
    changed: std::collections::HashSet<usize>,
    change_log: Vec<(String, String, String)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_style(&cc.egui_ctx);
        let mut app = App {
            tab: Tab::Tree,
            status: "drop a .bin here or use Open".into(),
            status_kind: Some(Kind::Info),
            ..Default::default()
        };
        names::count();
        app.status = format!("{} field names loaded — drop a .bin or click Open", names::count());
        app
    }

    fn set_status(&mut self, kind: Kind, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_kind = Some(kind);
    }

    fn open_path(&mut self, path: PathBuf) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                self.reset_loaded();
                self.path = Some(path.clone());
                let is_dsss = bytes.len() >= 16 && &bytes[0..4] == b"DSSS";
                self.is_dsss = is_dsss;
                if is_dsss {
                    match dsss::parse_header(&bytes) {
                        Ok(h) => {
                            self.version = h.version;
                            self.flags = h.flags;
                            self.hash_valid = dsss::verify_file_hash(&bytes);
                            self.dec_len = dsss::decrypted_len(&bytes);
                            self.raw = Some(bytes);
                            self.set_status(
                                Kind::Info,
                                "encrypted DSSS save — enter a SteamID or Crack it",
                            );
                            if let Ok(id) = self.steamid_input.trim().parse::<u64>() {
                                self.decrypt_with(id);
                            }
                        }
                        Err(e) => {
                            self.raw = Some(bytes);
                            self.set_status(Kind::Err, format!("bad DSSS header: {e}"));
                        }
                    }
                } else {
                    self.raw = Some(bytes.clone());
                    self.payload = Some(bytes);
                    self.reparse();
                    self.set_status(
                        Kind::Ok,
                        format!("loaded raw RSZ payload ({}/{} roots)", self.ok_roots, self.roots.len()),
                    );
                }
            }
            Err(e) => self.set_status(Kind::Err, format!("open failed: {e}")),
        }
    }

    fn reset_loaded(&mut self) {
        self.payload = None;
        self.roots.clear();
        self.targets.clear();
        self.selected = None;
        self.steamid = None;
        self.dirty = false;
        self.ok_roots = 0;
    }

    fn decrypt_with(&mut self, id: u64) {
        let Some(raw) = self.raw.clone() else { return };
        match dsss::decrypt(&raw, id) {
            Ok(plain) => {
                self.steamid = Some(id);
                self.steamid_input = id.to_string();
                self.payload = Some(plain);
                self.dirty = false;
                self.reparse();
                self.set_status(
                    Kind::Ok,
                    format!(
                        "decrypted with {id} — {}/{} roots, {} editable fields",
                        self.ok_roots,
                        self.roots.len(),
                        self.targets.len()
                    ),
                );
            }
            Err(e) => self.set_status(Kind::Err, format!("decrypt failed: {e}")),
        }
    }

    fn reparse(&mut self) {
        if let Some(p) = &self.payload {
            self.roots = rsz::parse(p);
            self.ok_roots = self
                .roots
                .iter()
                .filter(|r| r.class.as_ref().map(|c| c.truncated.is_none()).unwrap_or(false))
                .count();
            self.targets = rsz::edit_targets(&self.roots);
        }
    }

    fn start_crack(&mut self) {
        let Some(raw) = self.raw.clone() else {
            self.set_status(Kind::Err, "open an encrypted .bin first");
            return;
        };
        if !self.is_dsss {
            self.set_status(Kind::Err, "this file is already a raw payload");
            return;
        }
        let done = Arc::new(AtomicU64::new(0));
        let total = dsss::steamid_count();
        let (tx, rx) = channel();
        let done_t = done.clone();
        std::thread::spawn(move || {
            let found = dsss::crack_with(&raw, |d| {
                done_t.store(d, Ordering::Relaxed);
            });
            let _ = tx.send(found);
        });
        self.crack = Some(CrackJob { done, total, rx });
        self.set_status(Kind::Info, "cracking the account-id space...");
    }

    fn poll_crack(&mut self) {
        let finished = if let Some(job) = &self.crack {
            match job.rx.try_recv() {
                Ok(found) => Some(found),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(None),
            }
        } else {
            None
        };
        if let Some(found) = finished {
            self.crack = None;
            match found {
                Some(id) => {
                    self.set_status(Kind::Ok, format!("cracked: SteamID64 {id}"));
                    self.decrypt_with(id);
                }
                None => self.set_status(Kind::Err, "crack failed — no SteamID found"),
            }
        }
    }

    fn watch_tick(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let Ok(meta) = std::fs::metadata(&path) else { return };
        let mt = meta.modified().ok();
        let sz = meta.len();
        if (mt, sz) == (self.watch_mtime, self.watch_size) {
            return;
        }
        if self.watch_pending == Some((mt, sz)) {
            self.watch_pending = None;
            self.watch_mtime = mt;
            self.watch_size = sz;
            self.reload_diff();
        } else {
            self.watch_pending = Some((mt, sz));
        }
    }

    fn reload_diff(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let Ok(bytes) = std::fs::read(&path) else { return };
        let new_payload = if self.is_dsss {
            let Some(id) = self.steamid else {
                self.set_status(Kind::Err, "watch needs a known SteamID (decrypt once first)");
                return;
            };
            match dsss::decrypt(&bytes, id) {
                Ok(p) => p,
                Err(_) => return,
            }
        } else {
            bytes.clone()
        };
        let old: std::collections::HashMap<usize, String> =
            self.targets.iter().map(|t| (t.off, t.text.clone())).collect();
        self.raw = Some(bytes);
        self.payload = Some(new_payload);
        self.reparse();
        self.changed.clear();
        let mut new_changes = Vec::new();
        for t in &self.targets {
            if let Some(prev) = old.get(&t.off) {
                if prev != &t.text {
                    self.changed.insert(t.off);
                    new_changes.push((short_path(&t.path), prev.clone(), t.text.clone()));
                }
            }
        }
        let n = new_changes.len();
        for c in new_changes.into_iter().rev() {
            self.change_log.insert(0, c);
        }
        self.change_log.truncate(500);
        self.dirty = false;
        if n > 0 {
            self.set_status(Kind::Ok, format!("game saved — {n} field(s) changed"));
        } else {
            self.set_status(Kind::Info, "file changed, no field diffs (structure shift?)");
        }
    }

    fn apply_edit(&mut self) {
        let Some(sel) = self.selected.clone() else { return };
        let Some(payload) = self.payload.as_mut() else { return };
        match rsz::set_target(payload, &sel.target, &sel.buf) {
            Ok(()) => {
                self.dirty = true;
                let path = sel.target.path.clone();
                self.reparse();
                if let Some(t) = self.targets.iter().find(|t| t.path == path) {
                    self.selected = Some(Selected {
                        buf: t.text.clone(),
                        target: t.clone(),
                    });
                }
                self.set_status(Kind::Ok, format!("set {} = {}", sel.target.path, sel.buf));
            }
            Err(e) => self.set_status(Kind::Err, e),
        }
    }

    fn save_encrypted(&mut self) {
        let Some(payload) = &self.payload else { return };
        let id = match self.resolve_save_id() {
            Some(id) => id,
            None => {
                self.set_status(Kind::Err, "need a SteamID to encrypt a loadable save");
                return;
            }
        };
        let default = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "edited.bin".into());
        if let Some(out) = rfd::FileDialog::new().set_file_name(default).save_file() {
            backup_file(&out);
            let file = dsss::build(payload, id);
            match std::fs::write(&out, &file) {
                Ok(()) => {
                    self.dirty = false;
                    self.set_status(Kind::Ok, format!("saved {} ({} bytes)", out.display(), file.len()));
                }
                Err(e) => self.set_status(Kind::Err, format!("write failed: {e}")),
            }
        }
    }

    fn export_payload(&mut self) {
        let Some(payload) = self.payload.clone() else { return };
        if let Some(out) = rfd::FileDialog::new().set_file_name("payload.bin").save_file() {
            match std::fs::write(&out, &payload) {
                Ok(()) => self.set_status(Kind::Ok, format!("exported raw payload -> {}", out.display())),
                Err(e) => self.set_status(Kind::Err, format!("write failed: {e}")),
            }
        }
    }

    fn resolve_save_id(&self) -> Option<u64> {
        self.steamid.or_else(|| self.steamid_input.trim().parse::<u64>().ok())
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl App {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll_crack();
        if self.crack.is_some() {
            ctx.request_repaint();
        }
        if self.watching {
            self.watch_tick();
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if let Some(p) = dropped.into_iter().next() {
            self.open_path(p);
        }

        self.top_bar(ui);
        self.bottom_bar(ui);

        match self.tab {
            Tab::Gameplay => {
                self.inspector_panel(ui);
                egui::CentralPanel::default().show_inside(ui, |ui| self.gameplay_view(ui));
            }
            Tab::Tree => {
                self.inspector_panel(ui);
                egui::CentralPanel::default().show_inside(ui, |ui| self.tree_view(ui));
            }
            Tab::Hex => {
                egui::CentralPanel::default().show_inside(ui, |ui| self.hex_view(ui));
            }
            Tab::Strings => {
                egui::CentralPanel::default().show_inside(ui, |ui| self.strings_view(ui));
            }
            Tab::Watch => {
                egui::CentralPanel::default().show_inside(ui, |ui| self.watch_view(ui));
            }
        }

        paint_drop_overlay(&ctx);
    }
}

impl App {
    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("top").show_inside(root, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("re9").strong().color(ACCENT).size(18.0));
                ui.label(RichText::new("save editor").color(MUTED).size(14.0));
                ui.separator();

                if ui.button("Open .bin").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("DSSS save / payload", &["bin"])
                        .pick_file()
                    {
                        self.open_path(p);
                    }
                }

                if let Some(p) = &self.path {
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let txt = if self.dirty { format!("{name}  *") } else { name };
                    let col = if self.dirty { NUM } else { MUTED };
                    ui.label(RichText::new(txt).color(col).monospace());
                }

                ui.separator();
                ui.label("SteamID");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.steamid_input)
                        .desired_width(170.0)
                        .hint_text("76561197…"),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(id) = self.steamid_input.trim().parse::<u64>() {
                        self.decrypt_with(id);
                    }
                }
                let busy = self.crack.is_some();
                if ui.add_enabled(self.is_dsss && !busy, egui::Button::new("Decrypt")).clicked() {
                    if let Ok(id) = self.steamid_input.trim().parse::<u64>() {
                        self.decrypt_with(id);
                    } else {
                        self.set_status(Kind::Err, "enter a numeric SteamID first");
                    }
                }
                if ui.add_enabled(self.is_dsss && !busy, egui::Button::new("Crack")).clicked() {
                    self.start_crack();
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let has_payload = self.payload.is_some();
                ui.selectable_value(&mut self.tab, Tab::Gameplay, "Gameplay");
                ui.selectable_value(&mut self.tab, Tab::Tree, "Tree");
                ui.selectable_value(&mut self.tab, Tab::Hex, "Hex");
                ui.selectable_value(&mut self.tab, Tab::Strings, "Strings");
                ui.selectable_value(&mut self.tab, Tab::Watch, "Watch");
                ui.separator();

                let can_watch = self.payload.is_some()
                    && self.path.is_some()
                    && (!self.is_dsss || self.steamid.is_some());
                let prev = self.watching;
                ui.add_enabled(can_watch, egui::Checkbox::new(&mut self.watching, "Watch file"));
                if self.watching && !prev {
                    if let Ok(meta) = self.path.as_ref().unwrap().metadata() {
                        self.watch_mtime = meta.modified().ok();
                        self.watch_size = meta.len();
                    }
                    self.tab = Tab::Watch;
                    self.set_status(Kind::Info, "watching file — save in-game to see diffs");
                }
                if !self.watching {
                    self.watch_pending = None;
                }
                if ui.add_enabled(has_payload, egui::Button::new("Save .bin (encrypt)")).clicked() {
                    self.save_encrypted();
                }
                if ui.add_enabled(has_payload, egui::Button::new("Export payload")).clicked() {
                    self.export_payload();
                }

                if let Some(job) = &self.crack {
                    let done = job.done.load(Ordering::Relaxed);
                    let frac = done as f32 / job.total as f32;
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .desired_width(220.0)
                            .text(format!("{:.1}%", frac * 100.0)),
                    );
                }
            });
            ui.add_space(4.0);
        });
    }

    fn bottom_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("status").show_inside(root, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let (glyph, col) = match self.status_kind {
                    Some(Kind::Ok) => ("●", GOOD),
                    Some(Kind::Err) => ("●", BAD),
                    _ => ("●", KEY),
                };
                ui.label(RichText::new(glyph).color(col));
                ui.label(RichText::new(&self.status).color(MUTED));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_dsss {
                        chip(ui, "flags", &format!("{:#x}", self.flags), KEY);
                        chip(ui, "v", &self.version.to_string(), KEY);
                        chip(
                            ui,
                            "hash",
                            if self.hash_valid { "ok" } else { "bad" },
                            if self.hash_valid { GOOD } else { BAD },
                        );
                    }
                    if self.payload.is_some() {
                        chip(ui, "roots", &format!("{}/{}", self.ok_roots, self.roots.len()), NUM);
                        chip(ui, "fields", &self.targets.len().to_string(), NUM);
                    }
                });
            });
            ui.add_space(2.0);
        });
    }

    fn inspector_panel(&mut self, root: &mut egui::Ui) {
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(340.0)
            .show_inside(root, |ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("INSPECTOR").color(MUTED).size(12.0).strong());
                ui.separator();
                let Some(sel) = self.selected.clone() else {
                    ui.add_space(20.0);
                    ui.label(RichText::new("select a field in the tree").color(MUTED));
                    return;
                };
                let t = &sel.target;
                ui.add_space(6.0);
                ui.label(RichText::new(short_path(&t.path)).color(KEY).monospace().strong());
                ui.add_space(8.0);
                egui::Grid::new("insp_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new("type").color(MUTED));
                    let tl = t.hint.clone().unwrap_or_else(|| rsz::type_name(t.ftype).to_string());
                    ui.label(RichText::new(tl).color(ACCENT).monospace());
                    ui.end_row();
                    ui.label(RichText::new("width").color(MUTED));
                    ui.label(RichText::new(format!("{} bytes", t.width)).monospace());
                    ui.end_row();
                    ui.label(RichText::new("offset").color(MUTED));
                    ui.label(RichText::new(format!("{:#x}", t.off)).color(NUM).monospace());
                    ui.end_row();
                    ui.label(RichText::new("current").color(MUTED));
                    ui.label(RichText::new(&t.text).color(STR).monospace());
                    ui.end_row();
                });
                ui.add_space(10.0);
                ui.label(RichText::new("new value").color(MUTED));
                let mut buf = sel.buf.clone();
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                if edit.changed() {
                    if let Some(s) = self.selected.as_mut() {
                        s.buf = buf.clone();
                    }
                }
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let apply = ui
                        .add(egui::Button::new(RichText::new("Apply").strong()).fill(ACCENT_DIM))
                        .clicked();
                    if apply || enter {
                        self.apply_edit();
                    }
                    if ui.button("Reset").clicked() {
                        if let Some(s) = self.selected.as_mut() {
                            s.buf = s.target.text.clone();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(match t.ftype {
                        2 => "bool: true / false",
                        0x10 => "comma-separated numbers, e.g. 1.5, 2.5, 3.5",
                        0xb | 0xc => "float value",
                        _ => "integer (decimal, or 0x… hex)",
                    })
                    .color(MUTED)
                    .size(11.0),
                );
            });
    }

    fn gameplay_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("gameplay").color(ACCENT).strong());
            ui.label(RichText::new("— curated editable fields (counts, items, stats…)").color(MUTED).size(12.0));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("filter").color(MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(f32::INFINITY)
                    .hint_text("narrow the list"),
            );
        });
        ui.separator();
        if self.payload.is_none() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("no payload loaded").color(MUTED)));
            return;
        }
        let q = self.search.trim().to_lowercase();
        let selected_off = self.selected.as_ref().map(|s| s.target.off);
        let changed = &self.changed;
        let mut clicked: Option<EditTarget> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut shown = 0;
            for t in &self.targets {
                if !is_gameplay(&t.path) {
                    continue;
                }
                if !q.is_empty() && !t.path.to_lowercase().contains(&q) {
                    continue;
                }
                shown += 1;
                let sel = selected_off == Some(t.off);
                if scalar_row(ui, &short_path(&t.path), t, sel, changed.contains(&t.off)).clicked() {
                    clicked = Some(t.clone());
                }
            }
            if shown == 0 {
                ui.label(RichText::new("nothing curated matched — try the Tree tab").color(MUTED));
            }
        });
        if let Some(t) = clicked {
            self.selected = Some(Selected { buf: t.text.clone(), target: t });
        }
    }

    fn watch_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let dot = if self.watching { GOOD } else { MUTED };
            ui.label(RichText::new("●").color(dot));
            ui.label(RichText::new("live watch").color(ACCENT).strong());
            ui.label(
                RichText::new(if self.watching {
                    "— save in-game; changes appear here, newest first"
                } else {
                    "— enable 'Watch file' in the toolbar"
                })
                .color(MUTED)
                .size(12.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("clear").clicked() {
                    self.change_log.clear();
                    self.changed.clear();
                }
            });
        });
        ui.separator();
        if self.change_log.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("no changes captured yet").color(MUTED).size(15.0));
                ui.add_space(4.0);
                ui.label(RichText::new("trigger a save in the game while watching").color(MUTED));
            });
            return;
        }
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(
            ui,
            row_h,
            self.change_log.len(),
            |ui, range| {
                for i in range {
                    let (path, old, new) = &self.change_log[i];
                    let mut job = egui::text::LayoutJob::default();
                    let mono = egui::FontId::monospace(13.0);
                    job.append(path, 0.0, fmt(KEY, &mono));
                    job.append("  ", 0.0, fmt(MUTED, &mono));
                    job.append(old, 0.0, fmt(BAD, &mono));
                    job.append(" → ", 0.0, fmt(MUTED, &mono));
                    job.append(new, 0.0, fmt(GOOD, &mono));
                    ui.label(job);
                }
            },
        );
    }

    fn tree_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("search").color(MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(f32::INFINITY)
                    .hint_text("filter fields by path, e.g. _SaveCount or _Stock"),
            );
        });
        ui.separator();

        if self.payload.is_none() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("no payload loaded").color(MUTED).size(16.0));
                ui.add_space(4.0);
                ui.label(RichText::new("open an encrypted DSSS save and Crack/Decrypt it,").color(MUTED));
                ui.label(RichText::new("or drop an already-decrypted payload.").color(MUTED));
            });
            return;
        }

        let q = self.search.trim().to_lowercase();
        let mut clicked: Option<EditTarget> = None;
        let selected_off = self.selected.as_ref().map(|s| s.target.off);
        let changed = &self.changed;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !q.is_empty() {
                let mut shown = 0;
                for t in &self.targets {
                    if !t.path.to_lowercase().contains(&q) {
                        continue;
                    }
                    shown += 1;
                    if shown > 400 {
                        ui.label(RichText::new("… more results, refine the search").color(MUTED));
                        break;
                    }
                    let sel = selected_off == Some(t.off);
                    if scalar_row(ui, &short_path(&t.path), t, sel, changed.contains(&t.off)).clicked() {
                        clicked = Some(t.clone());
                    }
                }
                if shown == 0 {
                    ui.label(RichText::new("no match").color(MUTED));
                }
            } else {
                let roots = &self.roots;
                for (i, root) in roots.iter().enumerate() {
                    match &root.class {
                        Ok(class) => {
                            let tag = if class.truncated.is_some() { "  [partial]" } else { "" };
                            let title = format!(
                                "ROOT[{i}]  {}  ({} fields){tag}",
                                clean(class.hash),
                                class.fields.len()
                            );
                            let path = format!("R{i}#{:08x}", root.native_hash);
                            egui::CollapsingHeader::new(RichText::new(title).color(ACCENT).strong())
                                .id_salt(("root", i))
                                .default_open(i == 0)
                                .show(ui, |ui| {
                                    show_class(ui, &path, class, selected_off, &mut clicked, changed);
                                });
                        }
                        Err(_) => {
                            ui.label(
                                RichText::new(format!("ROOT[{i}]  parse failed (string-scan fallback)"))
                                    .color(BAD)
                                    .italics(),
                            );
                        }
                    }
                }
            }
        });

        if let Some(t) = clicked {
            self.selected = Some(Selected {
                buf: t.text.clone(),
                target: t,
            });
        }
    }

    fn hex_view(&mut self, ui: &mut egui::Ui) {
        let Some(payload) = self.payload.clone() else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("no payload loaded").color(MUTED)));
            return;
        };
        ui.add_space(6.0);
        let mut goto: Option<usize> = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("goto offset").color(MUTED));
            let r = ui.add(egui::TextEdit::singleline(&mut self.hex_goto).desired_width(120.0).hint_text("0x…"));
            if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("go").clicked() {
                if let Ok(off) = edit::parse_offset(&self.hex_goto) {
                    goto = Some(off);
                }
            }
            if let Some(sel) = &self.selected {
                if ui.button(format!("jump to selected @{:#x}", sel.target.off)).clicked() {
                    goto = Some(sel.target.off);
                }
            }
            ui.label(RichText::new(format!("{} bytes", payload.len())).color(MUTED));
        });
        ui.separator();

        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        let rows = payload.len().div_ceil(16);
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if let Some(off) = goto {
            let target_row = off / 16;
            area = area.vertical_scroll_offset(target_row as f32 * row_h);
        }
        let hi = self.selected.as_ref().map(|s| (s.target.off, s.target.off + s.target.width as usize));
        area.show_rows(ui, row_h, rows, |ui, range| {
            for row in range {
                hex_row(ui, &payload, row * 16, hi);
            }
        });
    }

    fn strings_view(&mut self, ui: &mut egui::Ui) {
        let Some(payload) = self.payload.clone() else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("no payload loaded").color(MUTED)));
            return;
        };
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("filter").color(MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.str_filter)
                    .desired_width(f32::INFINITY)
                    .hint_text("substring filter"),
            );
        });
        ui.separator();
        let f = self.str_filter.trim().to_lowercase();
        let all = edit::utf16_strings(&payload, 3);
        let filtered: Vec<_> = all
            .iter()
            .filter(|(_, s)| f.is_empty() || s.to_lowercase().contains(&f))
            .collect();
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        ui.label(RichText::new(format!("{} strings", filtered.len())).color(MUTED));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(
            ui,
            row_h,
            filtered.len(),
            |ui, range| {
                for idx in range {
                    let (off, s) = filtered[idx];
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{off:#08x}")).color(MUTED).monospace());
                        ui.label(RichText::new(s).color(STR).monospace());
                    });
                }
            },
        );
    }
}

fn show_class(
    ui: &mut egui::Ui,
    path: &str,
    class: &Class,
    selected_off: Option<usize>,
    clicked: &mut Option<EditTarget>,
    changed: &Changed,
) {
    for f in &class.fields {
        let child = format!("{path}.{}", names::name_for(f.hash));
        let hint = re9::schema::field_type(class.hash, f.hash);
        show_value(ui, &child, &clean(f.hash), f.ftype, &f.value, selected_off, clicked, hint, changed);
    }
    if let Some(t) = &class.truncated {
        ui.label(RichText::new(format!("truncated: {t}")).color(BAD).italics().size(11.0));
    }
}

fn show_value(
    ui: &mut egui::Ui,
    path: &str,
    label: &str,
    ftype: i32,
    v: &Value,
    selected_off: Option<usize>,
    clicked: &mut Option<EditTarget>,
    hint: Option<&str>,
    changed: &Changed,
) {
    match v {
        Value::Scalar { off, ftype, width, text } => {
            let t = EditTarget {
                path: path.to_string(),
                off: *off,
                ftype: *ftype,
                width: *width,
                text: text.clone(),
                hint: hint.map(|s| s.to_string()),
            };
            let sel = selected_off == Some(*off);
            if scalar_row(ui, label, &t, sel, changed.contains(off)).clicked() {
                *clicked = Some(t);
            }
        }
        Value::Str { off, s } => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).color(KEY).monospace());
                ui.label(RichText::new(format!("{s:?}")).color(STR).monospace());
                ui.label(RichText::new(format!("@{off:#x}")).color(MUTED).size(10.0));
            });
        }
        Value::StructBytes { off, bytes } => {
            let decoded = hint.and_then(|h| rsz::decode_struct(h, bytes));
            let editable = hint.map(rsz::struct_editable).unwrap_or(false);
            if let (Some(dec), true) = (decoded.clone(), editable) {
                let t = EditTarget {
                    path: path.to_string(),
                    off: *off,
                    ftype: 0x10,
                    width: bytes.len() as u8,
                    text: dec.clone(),
                    hint: hint.map(|s| s.to_string()),
                };
                let sel = selected_off == Some(*off);
                if scalar_row(ui, label, &t, sel, changed.contains(off)).clicked() {
                    *clicked = Some(t);
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).color(KEY).monospace());
                    if let Some(dec) = decoded {
                        ui.label(RichText::new(hint.unwrap()).color(ACCENT).monospace().size(11.0));
                        ui.label(RichText::new(dec).color(NUM).monospace());
                    } else {
                        let hex: String = bytes.iter().take(12).map(|b| format!("{b:02x}")).collect();
                        let more = if bytes.len() > 12 { "…" } else { "" };
                        let ty = hint.map(|h| format!("{h} ")).unwrap_or_default();
                        ui.label(RichText::new(format!("{ty}Struct[{}] {hex}{more}", bytes.len())).color(MUTED).monospace());
                    }
                    ui.label(RichText::new(format!("@{off:#x}")).color(MUTED).size(10.0));
                });
            }
        }
        Value::Array { member_type, items } => {
            let elem = hint.unwrap_or(rsz::type_name(*member_type));
            let title = format!("{label}  Array<{}>[{}]", elem, items.len());
            egui::CollapsingHeader::new(RichText::new(title).color(NUM))
                .id_salt(path)
                .default_open(false)
                .show(ui, |ui| {
                    for (i, it) in items.iter().enumerate() {
                        let child = format!("{path}[{i}]");
                        show_value(ui, &child, &format!("[{i}]"), *member_type, it, selected_off, clicked, hint, changed);
                    }
                });
        }
        Value::Class(c) => {
            let title = format!("{label}  {} ({} fields)", clean(c.hash), c.fields.len());
            egui::CollapsingHeader::new(RichText::new(title).color(KEY))
                .id_salt(path)
                .default_open(false)
                .show(ui, |ui| {
                    show_class(ui, path, c, selected_off, clicked, changed);
                });
        }
    }
    let _ = ftype;
}

fn scalar_row(ui: &mut egui::Ui, label: &str, t: &EditTarget, selected: bool, changed: bool) -> egui::Response {
    let value_col = match t.ftype {
        2 => {
            if t.text == "true" { GOOD } else { BAD }
        }
        0xb | 0xc => NUM,
        _ => NUM,
    };
    let type_label = t.hint.clone().unwrap_or_else(|| rsz::type_name(t.ftype).to_string());
    let enum_name = rsz::enum_label(t.hint.as_deref(), &t.text);
    let mut job = egui::text::LayoutJob::default();
    let mono = egui::FontId::monospace(13.0);
    if changed {
        job.append("● ", 0.0, fmt(ACCENT, &mono));
    }
    job.append(label, 0.0, fmt(KEY, &mono));
    job.append("  ", 0.0, fmt(MUTED, &mono));
    job.append(&type_label, 0.0, fmt(ACCENT, &mono));
    job.append(" = ", 0.0, fmt(MUTED, &mono));
    let vcol = if changed { ACCENT } else { value_col };
    if let Some(name) = enum_name {
        job.append(name, 0.0, fmt(if changed { ACCENT } else { STR }, &mono));
        job.append(&format!("({})", t.text), 0.0, fmt(MUTED, &mono));
    } else {
        job.append(&t.text, 0.0, fmt(vcol, &mono));
    }
    ui.selectable_label(selected, job)
}

fn fmt(color: Color32, font: &egui::FontId) -> egui::text::TextFormat {
    egui::text::TextFormat {
        color,
        font_id: font.clone(),
        ..Default::default()
    }
}

fn hex_row(ui: &mut egui::Ui, d: &[u8], base: usize, hi: Option<(usize, usize)>) {
    let mono = egui::FontId::monospace(13.0);
    let mut job = egui::text::LayoutJob::default();
    job.append(&format!("{base:08x}  "), 0.0, fmt(MUTED, &mono));
    for j in 0..16 {
        let idx = base + j;
        if idx < d.len() {
            let inside = hi.map(|(a, b)| idx >= a && idx < b).unwrap_or(false);
            let col = if inside { ACCENT } else { Color32::from_rgb(0xcf, 0xcf, 0xe0) };
            job.append(&format!("{:02x} ", d[idx]), 0.0, fmt(col, &mono));
        } else {
            job.append("   ", 0.0, fmt(MUTED, &mono));
        }
        if j == 7 {
            job.append(" ", 0.0, fmt(MUTED, &mono));
        }
    }
    job.append(" ", 0.0, fmt(MUTED, &mono));
    for j in 0..16 {
        let idx = base + j;
        if idx < d.len() {
            let c = d[idx];
            let ch = if (0x20..0x7f).contains(&c) { c as char } else { '.' };
            let inside = hi.map(|(a, b)| idx >= a && idx < b).unwrap_or(false);
            let col = if inside { ACCENT } else { MUTED };
            job.append(&ch.to_string(), 0.0, fmt(col, &mono));
        }
    }
    ui.label(job);
}

fn chip(ui: &mut egui::Ui, key: &str, val: &str, col: Color32) {
    ui.label(RichText::new(format!("{key}:")).color(MUTED).size(11.0));
    ui.label(RichText::new(val).color(col).monospace().size(11.0));
    ui.add_space(6.0);
}

fn clean(hash: u32) -> String {
    let s = names::name_for(hash);
    match s.split_once('#') {
        Some((name, _)) => name.to_string(),
        None => s,
    }
}

fn backup_file(path: &std::path::Path) {
    if path.exists() {
        let mut b = path.as_os_str().to_os_string();
        b.push(".bak");
        let _ = std::fs::copy(path, std::path::PathBuf::from(b));
    }
}

fn short_path(p: &str) -> String {
    p.split('.')
        .map(|seg| seg.split('#').next().unwrap_or(seg))
        .collect::<Vec<_>>()
        .join(".")
}

fn paint_drop_overlay(ctx: &egui::Context) {
    let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
    if !hovering {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("drop_overlay"),
    ));
    let rect = ctx.content_rect();
    painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 160));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "drop the .bin to open",
        egui::FontId::proportional(28.0),
        ACCENT,
    );
}

fn install_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(0x16, 0x16, 0x1d);
    visuals.window_fill = Color32::from_rgb(0x1b, 0x1b, 0x24);
    visuals.extreme_bg_color = Color32::from_rgb(0x10, 0x10, 0x16);
    visuals.selection.bg_fill = ACCENT_DIM.linear_multiply(0.9);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.hyperlink_color = ACCENT;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    ctx.set_global_style(style);
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 520.0])
            .with_title("re9 save editor"),
        ..Default::default()
    };
    let initial = std::env::args().nth(1).map(PathBuf::from);
    eframe::run_native(
        "re9_editor",
        options,
        Box::new(|cc| {
            let mut app = App::new(cc);
            if let Some(p) = initial {
                app.open_path(p);
            }
            Ok(Box::new(app))
        }),
    )
}
