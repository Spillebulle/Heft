//! The window's state and its shell: top bar, sidebar, options strip, content,
//! status bar (§6.1 of the style guide).

use crate::icons::Icon;
use crate::theme::{self, Mode, Palette, metrics, text};
use crate::treemap::Layout;
use crate::views::{self, Action, Reports, Row};
use crate::widgets::{self, Kind};
use egui::{Align, Layout as ELayout, RichText, Sense, Vec2, pos2, vec2};
use heft_scan::remove::{self, How};
use heft_scan::units::{Units, format_count, format_duration, format_size};
use heft_scan::volumes::{self, Volume};
use heft_scan::{NONE, NodeId, Options, Scan};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Tree,
    Treemap,
    Types,
    Largest,
    Age,
}

impl View {
    pub const ALL: [View; 5] = [
        View::Tree,
        View::Treemap,
        View::Types,
        View::Largest,
        View::Age,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemePref {
    System,
    Dark,
    Light,
}

/// A removal the user has asked for and not yet confirmed.
struct Confirm {
    nodes: Vec<NodeId>,
    title: String,
    detail: String,
    permanent: bool,
}

pub struct App {
    pub scan: Option<Scan>,
    /// What the current scan was started on, for the sidebar's selection.
    pub scanned: Vec<PathBuf>,
    pub volumes: Vec<Volume>,
    pub folders: Vec<PathBuf>,
    pub view: View,
    /// The folder the views are about (the treemap's zoom root).
    pub location: NodeId,
    pub selected: Option<NodeId>,
    pub expanded: HashSet<NodeId>,
    pub logical: bool,
    pub units: Units,
    pub theme: ThemePref,
    system_dark: bool,

    pub rows: Vec<Row>,
    pub rows_stale: bool,
    pub treemap: Option<Layout>,
    pub treemap_stale: bool,
    reports: Arc<Mutex<Option<Arc<Reports>>>>,
    reports_running: Arc<std::sync::atomic::AtomicBool>,

    seen_generation: u64,
    last_refresh: Instant,
    confirm: Option<Confirm>,
    message: Option<(String, Instant)>,
    removing: Option<Arc<Mutex<Option<remove::Outcome>>>>,
    settings_open: bool,
    /// Focus regained: rescan the folder handed to Brokey (contract: rescan
    /// on exit and on focus). Unused until the handoff is wired.
    pub was_focused: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, start: crate::Start) -> Self {
        crate::install_fonts(&cc.egui_ctx);
        let mut app = App {
            scan: None,
            scanned: Vec::new(),
            volumes: volumes::list(),
            folders: Vec::new(),
            view: View::Tree,
            location: NONE,
            selected: None,
            expanded: HashSet::new(),
            logical: false,
            units: Units::platform_default(),
            theme: ThemePref::System,
            system_dark: true,
            rows: Vec::new(),
            rows_stale: true,
            treemap: None,
            treemap_stale: true,
            reports: Arc::new(Mutex::new(None)),
            reports_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            seen_generation: u64::MAX,
            last_refresh: Instant::now(),
            confirm: None,
            message: None,
            removing: None,
            settings_open: false,
            was_focused: true,
        };
        if let Some(s) = cc.storage {
            if let Some(u) = s.get_string("units").and_then(|k| Units::from_key(&k)) {
                app.units = u;
            }
            app.logical = s.get_string("logical").as_deref() == Some("true");
            app.theme = match s.get_string("theme").as_deref() {
                Some("dark") => ThemePref::Dark,
                Some("light") => ThemePref::Light,
                _ => ThemePref::System,
            };
            if let Some(f) = s.get_string("folders") {
                app.folders = f
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(PathBuf::from)
                    .collect();
            }
        }
        app.view = match start.view.as_deref() {
            Some("treemap") => View::Treemap,
            Some("types") => View::Types,
            Some("largest") => View::Largest,
            Some("age") => View::Age,
            _ => View::Tree,
        };
        if !start.roots.is_empty() {
            app.start_scan(start.roots);
        }
        app
    }

    pub fn mode(&self) -> Mode {
        match self.theme {
            ThemePref::Dark => Mode::Dark,
            ThemePref::Light => Mode::Light,
            ThemePref::System => {
                if self.system_dark {
                    Mode::Dark
                } else {
                    Mode::Light
                }
            }
        }
    }

    fn palette(&self) -> Palette {
        Palette::of(self.mode())
    }

    pub fn start_scan(&mut self, roots: Vec<PathBuf>) {
        if let Some(s) = &self.scan {
            s.cancel();
        }
        self.scan = Some(Scan::start(roots.clone(), Options::default()));
        self.scanned = roots;
        self.location = NONE;
        self.selected = None;
        self.expanded.clear();
        self.rows_stale = true;
        self.treemap = None;
        *self.reports.lock().unwrap() = None;
        self.seen_generation = u64::MAX;
    }

    pub fn select(&mut self, n: NodeId) {
        self.selected = Some(n);
        if let Some(scan) = &self.scan {
            let t = scan.tree();
            self.location = if t.is_dir(n) { n } else { t.parent(n) };
        }
    }

    pub fn trash_label() -> &'static str {
        if cfg!(windows) {
            "Move to Recycle Bin"
        } else {
            "Move to Trash"
        }
    }

    pub fn act(&mut self, n: NodeId, a: Action) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        match a {
            Action::ZoomIn => {
                self.location = n;
                self.selected = None;
                self.view = View::Treemap;
            }
            Action::Rescan => {
                scan.rescan(n);
                self.rows_stale = true;
                self.treemap_stale = true;
            }
            Action::Show => show_in_file_manager(&scan.fs_path(n), scan.tree().is_dir(n)),
            Action::Remove => self.ask_remove(vec![n], false),
        }
    }

    fn ask_remove(&mut self, nodes: Vec<NodeId>, permanent: bool) {
        let Some(scan) = &self.scan else { return };
        let t = scan.tree();
        let bytes: u64 = nodes.iter().map(|&n| t.disk(n)).sum();
        let files: u64 = nodes.iter().map(|&n| t.files(n) as u64).sum();
        let name = if nodes.len() == 1 {
            t.display_name(nodes[0])
        } else {
            format!("{} items", nodes.len())
        };
        let size = format_size(bytes, self.units);
        let (title, detail) = if permanent {
            (
                format!("Delete {name} permanently?"),
                format!(
                    "{size} in {} files will be deleted for good. This cannot be undone.",
                    format_count(files.max(1))
                ),
            )
        } else {
            let place = if cfg!(windows) {
                "Recycle Bin"
            } else {
                "Trash"
            };
            (
                format!("{} {name}?", Self::trash_label()),
                format!(
                    "{size} in {} files. You can restore it from the {place} until it is emptied.",
                    format_count(files.max(1))
                ),
            )
        };
        drop(t);
        self.confirm = Some(Confirm {
            nodes,
            title,
            detail,
            permanent,
        });
    }

    fn run_remove(&mut self, nodes: Vec<NodeId>, how: How) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let slot = Arc::new(Mutex::new(None));
        let out = slot.clone();
        std::thread::spawn(move || {
            let r = remove::remove(&scan, &nodes, how);
            *out.lock().unwrap() = Some(r);
        });
        self.removing = Some(slot);
        if let Some(s) = self.selected
            && self
                .scan
                .as_ref()
                .is_some_and(|sc| sc.tree().parent(s) != NONE)
        {
            self.selected = self.scan.as_ref().map(|sc| sc.tree().parent(s));
        }
    }

    pub fn reports_ready(&self) -> Option<Arc<Reports>> {
        let g = self.reports.lock().ok()?;
        g.as_ref()
            .filter(|r| r.root == self.location && r.logical == self.logical)
            .cloned()
    }

    fn maybe_compute_reports(&mut self, generation: u64) {
        let needs = matches!(self.view, View::Types | View::Largest | View::Age);
        if !needs || self.location == NONE {
            return;
        }
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let fresh = self.reports.lock().unwrap().as_ref().is_some_and(|r| {
            r.root == self.location && r.logical == self.logical && r.generation == generation
        });
        if fresh
            || self
                .reports_running
                .load(std::sync::atomic::Ordering::Acquire)
        {
            return;
        }
        // While scanning, refresh at most every second; the pass is cheap but
        // not free on a big tree.
        if !scan.is_finished()
            && self.reports.lock().unwrap().is_some()
            && self.last_refresh.elapsed() < Duration::from_secs(1)
        {
            return;
        }
        self.reports_running
            .store(true, std::sync::atomic::Ordering::Release);
        let (root, logical, slot, running) = (
            self.location,
            self.logical,
            self.reports.clone(),
            self.reports_running.clone(),
        );
        std::thread::spawn(move || {
            let r = {
                let t = scan.tree();
                views::compute_reports(&t, root, generation, logical)
            };
            *slot.lock().unwrap() = Some(Arc::new(r));
            running.store(false, std::sync::atomic::Ordering::Release);
        });
    }

    fn save(&self, storage: &mut dyn eframe::Storage) {
        storage.set_string("units", self.units.key().into());
        storage.set_string("logical", self.logical.to_string());
        storage.set_string(
            "theme",
            match self.theme {
                ThemePref::System => "system",
                ThemePref::Dark => "dark",
                ThemePref::Light => "light",
            }
            .into(),
        );
        storage.set_string(
            "folders",
            self.folders
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(t) = ctx.system_theme() {
            self.system_dark = t == egui::Theme::Dark;
        }
        let mode = self.mode();
        let p = self.palette();
        crate::apply_visuals(ctx, p, mode);

        // Pick up what the walker did since the last frame, throttled.
        if let Some(scan) = self.scan.clone() {
            if self.location == NONE
                && let Some(t) = scan.try_tree()
                && let Some(&r) = t.roots().first()
            {
                self.location = r;
                for &r in t.roots() {
                    self.expanded.insert(r);
                }
            }
            let g = scan.generation();
            let finished = scan.is_finished();
            if g != self.seen_generation
                && (finished || self.last_refresh.elapsed() > Duration::from_millis(250))
            {
                self.seen_generation = g;
                self.last_refresh = Instant::now();
                self.rows_stale = true;
                self.treemap_stale = true;
            }
            self.maybe_compute_reports(g);
            if !finished
                || self
                    .reports_running
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
        if let Some(slot) = &self.removing {
            let done = slot.lock().unwrap().take();
            if let Some(out) = done {
                let msg = if out.failed.is_empty() {
                    format!(
                        "Removed {} item{}.",
                        out.removed.len(),
                        if out.removed.len() == 1 { "" } else { "s" }
                    )
                } else {
                    let (path, why) = &out.failed[0];
                    format!("Could not remove {}: {why}", path.display())
                };
                self.message = Some((msg, Instant::now()));
                self.removing = None;
                self.rows_stale = true;
                self.treemap_stale = true;
                *self.reports.lock().unwrap() = None;
                self.volumes = volumes::list();
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }

        keyboard(ctx, self);
        top_bar(ctx, p, self);
        status_bar(ctx, p, self);
        sidebar(ctx, p, self);
        options_strip(ctx, p, self);
        if self.view == View::Treemap && self.scan.is_some() {
            crate::details::panel(ctx, p, self);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.window))
            .show(ctx, |ui| match self.view {
                View::Tree => views::tree_view(ui, p, self),
                View::Treemap => views::treemap_view(ui, p, self),
                View::Types => views::types_view(ui, p, self),
                View::Largest => views::largest_view(ui, p, self),
                View::Age => views::age_view(ui, p, self),
            });
        confirm_dialog(ctx, p, self);
        settings_dialog(ctx, p, self);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        App::save(self, storage);
    }
}

fn keyboard(ctx: &egui::Context, app: &mut App) {
    if ctx.wants_keyboard_input() || app.confirm.is_some() {
        return;
    }
    let (up, del, shift, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Backspace),
            i.key_pressed(egui::Key::Delete),
            i.modifiers.shift,
            i.key_pressed(egui::Key::Escape),
        )
    });
    if esc {
        app.selected = None;
    }
    if up {
        go_up(app);
    }
    if del
        && let Some(n) = app.selected
        && let Some(scan) = &app.scan
        && remove::refusal(scan, n).is_none()
    {
        app.ask_remove(vec![n], shift);
    }
}

fn go_up(app: &mut App) {
    if let Some(scan) = &app.scan {
        let p = scan.tree().parent(app.location);
        if p != NONE {
            app.selected = Some(app.location);
            app.location = p;
        }
    }
}

/// The version and licence at the foot of the sidebar with GitHub's mark
/// beside them, as the family's other apps have it (Tally's status bar,
/// Windup's rail). The mark opens the repository in the browser; nothing is
/// fetched.
fn version_line(ui: &mut egui::Ui, p: Palette) {
    const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
    ui.horizontal(|ui| {
        ui.add_space(metrics::S2);
        ui.label(
            RichText::new(format!("v{} · GPL-3.0", env!("CARGO_PKG_VERSION")))
                .size(text::TINY)
                .color(p.text_dim),
        );
        ui.add_space(metrics::S1);
        let side = 14.0;
        let (rect, r) = ui.allocate_exact_size(Vec2::splat(side + 4.0), Sense::click());
        let r = r
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "Source on GitHub
{REPOSITORY}"
            ));
        let ink = if r.hovered() { p.text } else { p.text_dim };
        crate::icons::draw(
            ui.painter(),
            egui::Rect::from_center_size(rect.center(), Vec2::splat(side)),
            Icon::Github,
            ink,
        );
        if r.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(REPOSITORY));
        }
    });
}

fn top_bar(ctx: &egui::Context, p: Palette, _app: &mut App) {
    egui::TopBottomPanel::top("top")
        .exact_height(metrics::MENU_BAR)
        .frame(
            egui::Frame::NONE
                .fill(p.chrome)
                .inner_margin(egui::Margin::symmetric(metrics::PAD_STRIP as i8, 0)),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            widgets::hairline_bottom(ui, p);
            ui.horizontal_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(metrics::MARK), Sense::hover());
                crate::art::paint_mark(ui.painter(), rect, p);
                ui.add_space(metrics::S2);
                ui.label(
                    RichText::new("Heft")
                        .font(theme::strong(text::HEADING))
                        .color(p.text_strong),
                );
            });
        });
}

fn sidebar(ctx: &egui::Context, p: Palette, app: &mut App) {
    egui::SidePanel::left("nav")
        .exact_width(metrics::SIDEBAR)
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(p.dock)
                .inner_margin(egui::Margin::symmetric(metrics::S2 as i8, 0)),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            widgets::hairline_right(ui, p);
            ui.with_layout(ELayout::bottom_up(Align::Min), |ui| {
                ui.add_space(metrics::S2);
                version_line(ui, p);
                ui.add_space(metrics::S1);
                if widgets::nav_row(ui, p, Icon::Gear, "Settings", None, false).clicked() {
                    app.settings_open = true;
                }
                ui.add_space(metrics::S2);
                widgets::hairline_across(ui, p, ui.min_rect().top());
                ui.with_layout(ELayout::top_down(Align::Min), |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            widgets::eyebrow(ui, p, "Disks");
                            let mut start = None;
                            for v in &app.volumes {
                                let on = app.scanned.len() == 1 && app.scanned[0] == v.path;
                                let note = format!(
                                    "{:.0}%",
                                    v.used() as f64 * 100.0 / v.total.max(1) as f64
                                );
                                let r =
                                    widgets::nav_row(ui, p, Icon::Drive, &v.label, Some(&note), on)
                                        .on_hover_text(format!(
                                            "{} used of {}, {} free",
                                            format_size(v.used(), app.units),
                                            format_size(v.total, app.units),
                                            format_size(v.free, app.units)
                                        ));
                                // A thin used/free rail under the label.
                                let rr = r.rect;
                                let rail = egui::Rect::from_min_size(
                                    pos2(
                                        rr.left() + metrics::PAD_PANEL + metrics::ICON + 10.0,
                                        rr.bottom() - 4.0,
                                    ),
                                    vec2(
                                        rr.width()
                                            - metrics::PAD_PANEL
                                            - metrics::ICON
                                            - 10.0
                                            - metrics::S2,
                                        2.0,
                                    ),
                                );
                                widgets::rail_at(
                                    ui.painter(),
                                    p,
                                    rail,
                                    Some(v.used() as f32 / v.total.max(1) as f32),
                                    p.text_dim,
                                );
                                if r.clicked() {
                                    start = Some(vec![v.path.clone()]);
                                }
                            }
                            if !app.folders.is_empty() {
                                widgets::eyebrow(ui, p, "Folders");
                            }
                            let mut forget = None;
                            for (i, f) in app.folders.iter().enumerate() {
                                let on = app.scanned.len() == 1 && &app.scanned[0] == f;
                                let label = f
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| f.display().to_string());
                                let r = widgets::nav_row(ui, p, Icon::Folder, &label, None, on)
                                    .on_hover_text(f.display().to_string());
                                if r.clicked() {
                                    start = Some(vec![f.clone()]);
                                }
                                r.context_menu(|ui| {
                                    if ui.button("Remove from this list").clicked() {
                                        forget = Some(i);
                                        ui.close();
                                    }
                                });
                            }
                            if let Some(i) = forget {
                                app.folders.remove(i);
                            }
                            ui.add_space(metrics::S1);
                            if widgets::nav_row(
                                ui,
                                p,
                                Icon::AddFolder,
                                "Scan a folder…",
                                None,
                                false,
                            )
                            .clicked()
                                && let Some(dir) = rfd::FileDialog::new()
                                    .set_title("Choose a folder to scan")
                                    .pick_folder()
                            {
                                if !app.folders.contains(&dir) {
                                    app.folders.push(dir.clone());
                                }
                                start = Some(vec![dir]);
                            }
                            if let Some(roots) = start {
                                app.start_scan(roots);
                            }

                            widgets::eyebrow(ui, p, "Views");
                            for v in View::ALL {
                                if widgets::nav_row(
                                    ui,
                                    p,
                                    views::view_icon(v),
                                    views::view_label(v),
                                    None,
                                    app.view == v,
                                )
                                .clicked()
                                {
                                    app.view = v;
                                }
                            }
                        });
                });
            });
        });
}

fn options_strip(ctx: &egui::Context, p: Palette, app: &mut App) {
    egui::TopBottomPanel::top("options")
        .exact_height(metrics::OPTIONS_STRIP)
        .frame(
            egui::Frame::NONE
                .fill(p.chrome)
                .inner_margin(egui::Margin::symmetric(metrics::PAD_STRIP as i8, 0)),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            widgets::hairline_bottom(ui, p);
            ui.horizontal_centered(|ui| {
                let Some(scan) = app.scan.clone() else {
                    ui.label(
                        RichText::new("Choose a disk or a folder to scan.")
                            .size(text::BODY)
                            .color(p.text_dim),
                    );
                    return;
                };
                let (can_up, crumbs) = {
                    let t = scan.tree();
                    let mut chain = Vec::new();
                    let mut n = app.location;
                    while n != NONE && (n as usize) < t.len() {
                        chain.push((n, t.display_name(n)));
                        n = t.parent(n);
                    }
                    chain.reverse();
                    (
                        app.location != NONE && t.parent(app.location) != NONE,
                        chain,
                    )
                };
                if widgets::icon_button(ui, p, Icon::Up, "Up one folder (Backspace)", can_up)
                    .clicked()
                {
                    go_up(app);
                }
                ui.add_space(metrics::S1);
                // Breadcrumb: the last few segments, each clickable.
                let shown = crumbs.len().saturating_sub(5);
                if shown > 0 {
                    ui.label(RichText::new("…").size(text::BODY).color(p.text_dim));
                }
                let last = crumbs.len().saturating_sub(1);
                for (i, (n, name)) in crumbs.iter().enumerate().skip(shown) {
                    let strong = i == last;
                    let r = ui.add(
                        egui::Label::new(
                            RichText::new(widgets::ellipsize(name, 32))
                                .size(text::BODY)
                                .color(if strong { p.text_strong } else { p.text_muted }),
                        )
                        .sense(Sense::click()),
                    );
                    if r.clicked() {
                        app.location = *n;
                    }
                    if !strong {
                        ui.label(RichText::new("/").size(text::BODY).color(p.text_dim));
                    }
                }

                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if let Some(i) =
                        widgets::segmented(ui, p, &["On disk", "Logical"], app.logical as usize)
                    {
                        app.logical = i == 1;
                        app.rows_stale = true;
                        app.treemap_stale = true;
                    }
                    ui.add_space(metrics::S2);
                    let sel = app.selected;
                    let refusal = sel.map(|n| remove::refusal(&scan, n));
                    let can_remove = matches!(refusal, Some(None)) && app.removing.is_none();
                    let r = widgets::button_with(
                        ui,
                        p,
                        Some(Icon::Trash),
                        "Delete",
                        Kind::Outlined,
                        can_remove,
                    );
                    let r = match (sel, &refusal) {
                        (None, _) => r.on_hover_text("Select something to delete."),
                        (_, Some(Some(why))) => r.on_hover_text(why.as_str()),
                        _ if app.removing.is_some() => {
                            r.on_hover_text("Already removing something.")
                        }
                        _ => r.on_hover_text(format!(
                            "{} (Delete). Shift+Delete deletes permanently.",
                            App::trash_label()
                        )),
                    };
                    if r.clicked()
                        && let Some(n) = sel
                    {
                        app.ask_remove(vec![n], false);
                    }
                    if let Some(n) = sel
                        && widgets::icon_button(ui, p, Icon::Folder, views::show_label(), true)
                            .clicked()
                    {
                        app.act(n, Action::Show);
                    }
                    if scan.is_finished() {
                        if widgets::button_with(
                            ui,
                            p,
                            Some(Icon::Refresh),
                            "Rescan",
                            Kind::Secondary,
                            true,
                        )
                        .on_hover_text("Read everything again from disk.")
                        .clicked()
                        {
                            let roots = app.scanned.clone();
                            app.start_scan(roots);
                        }
                    } else if widgets::button_with(
                        ui,
                        p,
                        Some(Icon::Close),
                        "Stop",
                        Kind::Secondary,
                        true,
                    )
                    .on_hover_text("Stop scanning. What has been read so far stays on screen.")
                    .clicked()
                    {
                        scan.cancel();
                    }
                });
            });
        });
}

fn status_bar(ctx: &egui::Context, p: Palette, app: &mut App) {
    egui::TopBottomPanel::bottom("status")
        .exact_height(metrics::STATUS_BAR)
        .frame(
            egui::Frame::NONE
                .fill(p.chrome)
                .inner_margin(egui::Margin::symmetric(metrics::PAD_STRIP as i8, 0)),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            widgets::hairline_top(ui, p);
            ui.horizontal_centered(|ui| {
                let line = match &app.scan {
                    None => "Ready".to_string(),
                    Some(scan) => {
                        let pr = scan.progress();
                        let mut s = if pr.finished {
                            let bytes = scan
                                .try_tree()
                                .map(|t| t.total(app.logical))
                                .unwrap_or(pr.bytes);
                            format!(
                                "{} entries · {} {} · {}",
                                format_count(pr.entries),
                                format_size(bytes, app.units),
                                if app.logical { "logical" } else { "on disk" },
                                format_duration(pr.elapsed)
                            )
                        } else {
                            format!(
                                "Scanning · {} entries · {} · {}",
                                format_count(pr.entries),
                                format_size(pr.bytes, app.units),
                                format_duration(pr.elapsed)
                            )
                        };
                        if pr.cancelled {
                            s.push_str(" · stopped early, so the totals are partial");
                        }
                        if pr.unreadable > 0 {
                            s.push_str(&format!(
                                " · {} folders could not be read and are not counted",
                                format_count(pr.unreadable)
                            ));
                        }
                        s.push_str(&format!(" · {}", heft_scan::walk::walker_name()));
                        s
                    }
                };
                ui.label(RichText::new(line).size(text::TINY).color(p.text_dim));
                if let Some((m, at)) = &app.message
                    && at.elapsed() < Duration::from_secs(8)
                {
                    ui.label(
                        RichText::new(format!(" · {m}"))
                            .size(text::TINY)
                            .color(p.text),
                    );
                }
                if app.removing.is_some() {
                    ui.label(RichText::new(" · Removing…").size(text::TINY).color(p.text));
                }
                if let Some(scan) = &app.scan
                    && !scan.is_finished()
                {
                    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                        // The total is unknown until the end: an empty rail.
                        widgets::rail(ui, p, 120.0, None);
                    });
                }
            });
        });
}

fn modal_frame(p: Palette, mode: Mode) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.popover)
        .stroke(egui::Stroke::new(metrics::HAIRLINE, p.line_popover))
        .corner_radius(metrics::RADIUS_MODAL)
        .shadow(theme::shadow::modal(mode))
        .inner_margin(egui::Margin::same(metrics::S4 as i8))
}

fn confirm_dialog(ctx: &egui::Context, p: Palette, app: &mut App) {
    let Some(c) = &app.confirm else { return };
    let mut close = false;
    let mut go: Option<How> = None;
    let mut escalate = false;
    let title = c.title.clone();
    let detail = c.detail.clone();
    let permanent = c.permanent;
    egui::Modal::new(egui::Id::new("confirm"))
        .frame(modal_frame(p, app.mode()))
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new(title)
                    .font(theme::strong(text::PAGE))
                    .color(p.text_strong),
            );
            ui.add_space(metrics::S2);
            ui.label(RichText::new(detail).size(text::BODY).color(p.text));
            ui.add_space(metrics::S4);
            ui.horizontal(|ui| {
                if !permanent
                    && widgets::button(ui, p, "Delete permanently…", Kind::Ghost, true).clicked()
                {
                    escalate = true;
                }
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    let label = if permanent {
                        "Delete permanently"
                    } else {
                        App::trash_label()
                    };
                    if widgets::button(ui, p, label, Kind::Primary, true).clicked() {
                        go = Some(if permanent {
                            How::Permanently
                        } else {
                            How::Trash
                        });
                    }
                    if widgets::button(ui, p, "Cancel", Kind::Secondary, true).clicked() {
                        close = true;
                    }
                });
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
    if escalate {
        let nodes = app.confirm.take().map(|c| c.nodes).unwrap_or_default();
        app.ask_remove(nodes, true);
    } else if let Some(how) = go {
        let nodes = app.confirm.take().map(|c| c.nodes).unwrap_or_default();
        app.run_remove(nodes, how);
    } else if close {
        app.confirm = None;
    }
}

fn settings_dialog(ctx: &egui::Context, p: Palette, app: &mut App) {
    if !app.settings_open {
        return;
    }
    let mut close = false;
    egui::Modal::new(egui::Id::new("settings"))
        .frame(modal_frame(p, app.mode()))
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Settings")
                        .font(theme::strong(text::PAGE))
                        .color(p.text_strong),
                );
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, p, Icon::Close, "Close", true).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(metrics::S3);
            let row = |ui: &mut egui::Ui, label: &str, note: &str| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).size(text::BODY).color(p.text_strong));
                    ui.label(RichText::new(note).size(text::SMALL).color(p.text_dim));
                });
            };
            ui.horizontal(|ui| {
                row(ui, "Theme", "System follows your computer's setting.");
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    let cur = match app.theme {
                        ThemePref::System => 0,
                        ThemePref::Dark => 1,
                        ThemePref::Light => 2,
                    };
                    if let Some(i) = widgets::segmented(ui, p, &["System", "Dark", "Light"], cur) {
                        app.theme = [ThemePref::System, ThemePref::Dark, ThemePref::Light][i];
                        app.treemap_stale = true;
                    }
                });
            });
            ui.add_space(metrics::S3);
            ui.horizontal(|ui| {
                row(
                    ui,
                    "Size units",
                    "Windows-style is 1024 per step labelled KB, as Explorer shows.",
                );
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    let all = [Units::Decimal, Units::Binary, Units::Windows];
                    let cur = all.iter().position(|u| *u == app.units).unwrap_or(0);
                    if let Some(i) = widgets::segmented(ui, p, &["kB", "KiB", "Windows"], cur) {
                        app.units = all[i];
                    }
                });
            });
            ui.add_space(metrics::S3);
            ui.horizontal(|ui| {
                row(
                    ui,
                    "Sizes",
                    "On disk is what deleting frees. Logical is what programs read.",
                );
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if let Some(i) =
                        widgets::segmented(ui, p, &["On disk", "Logical"], app.logical as usize)
                    {
                        app.logical = i == 1;
                        app.rows_stale = true;
                        app.treemap_stale = true;
                    }
                });
            });
            ui.add_space(metrics::S4);
            ui.label(
                RichText::new(format!(
                    "Heft {} · GPL-3.0-or-later · icons by Lucide (ISC)",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(text::SMALL)
                .color(p.text_dim),
            );
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
    if close {
        app.settings_open = false;
    }
}

/// Opens the platform's file manager on `path`, selecting it where it can.
fn show_in_file_manager(path: &std::path::Path, is_dir: bool) {
    #[cfg(windows)]
    {
        let plain = path.to_string_lossy();
        let plain = plain.strip_prefix(r"\\?\").unwrap_or(&plain).to_string();
        let _ = is_dir;
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{plain}"))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let target = if is_dir {
            path.to_path_buf()
        } else {
            path.parent().map(|p| p.to_path_buf()).unwrap_or_default()
        };
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}
