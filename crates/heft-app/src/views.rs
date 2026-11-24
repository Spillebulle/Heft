//! The content area: one function per view, all reading the same tree and
//! sharing one selection and one location (the folder the views are about).

use crate::app::{App, View};
use crate::icons::{self, Icon};
use crate::theme::{self, Palette, metrics, text};
use crate::treemap;
use crate::widgets::{empty_state, rail_at};
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Vec2, pos2, vec2};
use heft_scan::kinds::FileKind;
use heft_scan::report::{self, AGE_BUCKETS, TypeStat};
use heft_scan::tree::flag;
use heft_scan::units::{format_count, format_size};
use heft_scan::{NONE, NodeId, Tree};

/// A row in the tree view: a node and how deep it is drawn.
#[derive(Clone, Copy)]
pub struct Row {
    pub node: NodeId,
    pub depth: u16,
}

/// The tree view's visible rows, in order.
pub fn tree_rows(app: &App, t: &Tree) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut stack: Vec<Row> = t
        .roots()
        .iter()
        .rev()
        .map(|&r| Row { node: r, depth: 0 })
        .collect();
    while let Some(r) = stack.pop() {
        if t.has(r.node, flag::REMOVED) {
            continue;
        }
        rows.push(r);
        if t.is_dir(r.node) && app.expanded.contains(&r.node) {
            for c in t.sorted_children(r.node, app.logical).into_iter().rev() {
                stack.push(Row {
                    node: c,
                    depth: r.depth + 1,
                });
            }
        }
    }
    rows
}

// Column widths, right to left after the name.
const COL_BAR: f32 = 120.0;
const COL_SIZE: f32 = 84.0;
const COL_SHARE: f32 = 56.0;
const COL_FILES: f32 = 84.0;
const COL_DATE: f32 = 88.0;
const INDENT: f32 = 16.0;

pub fn tree_view(ui: &mut egui::Ui, p: Palette, app: &mut App) {
    let Some(scan) = app.scan.clone() else {
        return no_scan(ui, p);
    };
    // Wait for the walker's write (microseconds) rather than skip the
    // frame: a skipped frame paints an empty panel, a visible flash.
    let t = scan.tree();
    if app.rows_stale {
        app.rows = tree_rows(app, &t);
        app.rows_stale = false;
    }
    let width = ui.available_width();
    let name_w =
        (width - COL_BAR - COL_SIZE - COL_SHARE - COL_FILES - COL_DATE - metrics::PAD_STRIP * 2.0)
            .max(160.0);

    // Header.
    let (head, _) = ui.allocate_exact_size(vec2(width, metrics::ROW), Sense::hover());
    ui.painter().rect_filled(head, 0.0, p.chrome);
    ui.painter().hline(
        head.left()..=head.right(),
        head.bottom(),
        Stroke::new(metrics::HAIRLINE, p.line),
    );
    let mut x = head.left() + metrics::PAD_STRIP;
    let font = FontId::proportional(text::SMALL);
    let hp = ui.painter();
    hp.text(
        pos2(x, head.center().y),
        Align2::LEFT_CENTER,
        "Name",
        font.clone(),
        p.text_dim,
    );
    x += name_w;
    for (label, w, right) in [
        ("Share of folder", COL_BAR, false),
        (if app.logical { "Size" } else { "On disk" }, COL_SIZE, true),
        ("%", COL_SHARE, true),
        ("Files", COL_FILES, true),
        ("Modified", COL_DATE, true),
    ] {
        if right {
            hp.text(
                pos2(x + w - metrics::S2, head.center().y),
                Align2::RIGHT_CENTER,
                label,
                font.clone(),
                p.text_dim,
            );
        } else {
            hp.text(
                pos2(x + metrics::S2, head.center().y),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                p.text_dim,
            );
        }
        x += w;
    }

    let rows = app.rows.clone();
    let mut toggle: Option<NodeId> = None;
    let mut select: Option<NodeId> = None;
    let mut action: Option<(NodeId, Action)> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, metrics::ROW, rows.len(), |ui, range| {
            for row in &rows[range] {
                let n = row.node;
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(width, metrics::ROW), Sense::click());
                let selected = app.selected == Some(n);
                if selected {
                    ui.painter().rect_filled(rect, 0.0, p.control);
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            rect.left_top(),
                            vec2(metrics::NAV_MARK_W, rect.height()),
                        ),
                        0.0,
                        p.accent,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, p.control_hover);
                }
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    rect.bottom(),
                    Stroke::new(metrics::HAIRLINE, p.line_soft),
                );

                let painter = ui.painter();
                let mut x = rect.left() + metrics::PAD_STRIP + row.depth as f32 * INDENT;
                let cy = rect.center().y;
                let is_dir = t.is_dir(n);
                let expandable = is_dir && !t.children(n).is_empty();
                let chevron = Rect::from_center_size(pos2(x + 7.0, cy), Vec2::splat(14.0));
                if expandable {
                    let open = app.expanded.contains(&n);
                    icons::draw(
                        painter,
                        chevron,
                        if open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        p.text_dim,
                    );
                }
                x += 18.0;
                let glyph = Rect::from_center_size(
                    pos2(x + metrics::ICON / 2.0, cy),
                    Vec2::splat(metrics::ICON - 2.0),
                );
                let ink = if selected { p.text_strong } else { p.text };
                let icon = if t.parent(n) == NONE {
                    Icon::Drive
                } else if is_dir {
                    Icon::Folder
                } else {
                    Icon::File
                };
                icons::draw(
                    painter,
                    glyph,
                    icon,
                    if selected {
                        p.text_strong
                    } else {
                        p.text_muted
                    },
                );
                x += metrics::ICON + metrics::S2;
                let name_right = rect.left() + metrics::PAD_STRIP + name_w - metrics::S2;
                let mut label = t.display_name(n);
                let note = row_note(&t, n);
                let galley_font = if selected {
                    theme::strong(text::BODY)
                } else {
                    FontId::proportional(text::BODY)
                };
                let avail = (name_right - x).max(20.0);
                let g = painter.layout_no_wrap(label.clone(), galley_font.clone(), ink);
                if g.size().x > avail {
                    while !label.is_empty()
                        && painter
                            .layout_no_wrap(format!("{label}…"), galley_font.clone(), ink)
                            .size()
                            .x
                            > avail
                    {
                        label.pop();
                    }
                    label.push('…');
                }
                let g = painter.layout_no_wrap(label, galley_font, ink);
                let gw = g.size().x;
                painter.galley(pos2(x, cy - g.size().y / 2.0), g, ink);
                if let Some(note) = note {
                    let nx = x + gw + metrics::S2;
                    if nx < name_right - 40.0 {
                        painter.text(
                            pos2(nx, cy),
                            Align2::LEFT_CENTER,
                            note,
                            FontId::proportional(text::SMALL),
                            p.text_dim,
                        );
                    }
                }

                // Share-of-parent bar.
                let mut cx = rect.left() + metrics::PAD_STRIP + name_w;
                let size = t.size(n, app.logical);
                let parent = t.parent(n);
                let share = if parent == NONE {
                    let total = t.total(app.logical).max(1);
                    size as f64 / total as f64
                } else {
                    size as f64 / t.size(parent, app.logical).max(1) as f64
                };
                let bar = Rect::from_min_size(
                    pos2(cx + metrics::S2, cy - 2.0),
                    vec2(COL_BAR - metrics::S2 * 2.0, 4.0),
                );
                rail_at(
                    painter,
                    p,
                    bar,
                    Some(share as f32),
                    if selected { p.accent } else { p.accent_dim },
                );
                cx += COL_BAR;
                let fig = FontId::monospace(text::TINY);
                let dim = if t.has(n, flag::COMPLETE) {
                    p.text
                } else {
                    p.text_dim
                };
                let size_text = if t.has(n, flag::UNREADABLE) {
                    "unknown".to_string()
                } else {
                    format_size(size, app.units)
                };
                painter.text(
                    pos2(cx + COL_SIZE - metrics::S2, cy),
                    Align2::RIGHT_CENTER,
                    size_text,
                    fig.clone(),
                    dim,
                );
                cx += COL_SIZE;
                painter.text(
                    pos2(cx + COL_SHARE - metrics::S2, cy),
                    Align2::RIGHT_CENTER,
                    format!("{:.1}", share * 100.0),
                    fig.clone(),
                    p.text_muted,
                );
                cx += COL_SHARE;
                if is_dir {
                    painter.text(
                        pos2(cx + COL_FILES - metrics::S2, cy),
                        Align2::RIGHT_CENTER,
                        format_count(t.files(n) as u64),
                        fig.clone(),
                        p.text_muted,
                    );
                }
                cx += COL_FILES;
                if t.mtime(n) > 0 {
                    painter.text(
                        pos2(cx + COL_DATE - metrics::S2, cy),
                        Align2::RIGHT_CENTER,
                        date(t.mtime(n)),
                        fig,
                        p.text_muted,
                    );
                }

                let resp = resp.on_hover_text(t.path(n).display().to_string());
                if resp.clicked() {
                    let on_chevron = resp
                        .interact_pointer_pos()
                        .is_some_and(|pt| pt.x < chevron.right() + 2.0);
                    if expandable && on_chevron {
                        toggle = Some(n);
                    }
                    select = Some(n);
                }
                if resp.double_clicked() && expandable {
                    toggle = Some(n);
                }
                resp.context_menu(|ui| {
                    if let Some(a) = context_menu(ui, app, n) {
                        action = Some((n, a));
                    }
                });
            }
        });
    drop(t);
    if let Some(n) = toggle {
        if !app.expanded.remove(&n) {
            app.expanded.insert(n);
        }
        app.rows_stale = true;
    }
    if let Some(n) = select {
        app.select(n);
    }
    if let Some((n, a)) = action {
        app.act(n, a);
    }
}

fn row_note(t: &Tree, n: NodeId) -> Option<&'static str> {
    if t.has(n, flag::UNREADABLE) {
        Some("could not be read")
    } else if t.has(n, flag::OTHER_FS) {
        Some("another filesystem, not scanned")
    } else if t.has(n, flag::VIRTUAL) {
        Some("system filesystem, not scanned")
    } else if t.has(n, flag::LINK) {
        Some("link, not followed")
    } else if t.has(n, flag::HARDLINK_DUP) {
        Some("hard link, counted elsewhere")
    } else if t.is_dir(n) && !t.has(n, flag::COMPLETE) {
        Some("scanning")
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Show,
    Rescan,
    Remove,
    ZoomIn,
}

fn context_menu(ui: &mut egui::Ui, app: &App, n: NodeId) -> Option<Action> {
    let mut out = None;
    let scan = app.scan.as_ref()?;
    let is_dir = scan.tree().is_dir(n);
    if is_dir && ui.button("Open in treemap").clicked() {
        out = Some(Action::ZoomIn);
        ui.close();
    }
    if ui.button(show_label()).clicked() {
        out = Some(Action::Show);
        ui.close();
    }
    let finished = scan.is_finished();
    if is_dir {
        let r = ui.add_enabled(finished, egui::Button::new("Rescan this folder"));
        let r = if finished {
            r
        } else {
            r.on_disabled_hover_text("Wait for the scan to finish.")
        };
        if r.clicked() {
            out = Some(Action::Rescan);
            ui.close();
        }
    }
    ui.separator();
    let refusal = heft_scan::remove::refusal(scan, n);
    let r = ui.add_enabled(refusal.is_none(), egui::Button::new(App::trash_label()));
    let r = match &refusal {
        Some(why) => r.on_disabled_hover_text(why),
        None => r,
    };
    if r.clicked() {
        out = Some(Action::Remove);
        ui.close();
    }
    out
}

pub fn show_label() -> &'static str {
    if cfg!(windows) {
        "Show in Explorer"
    } else {
        "Show in file manager"
    }
}

pub fn treemap_view(ui: &mut egui::Ui, p: Palette, app: &mut App) {
    let Some(scan) = app.scan.clone() else {
        return no_scan(ui, p);
    };
    // Wait for the walker's write (microseconds) rather than skip the
    // frame: a skipped frame paints an empty panel, a visible flash.
    let t = scan.tree();
    let area = ui.available_rect_before_wrap().shrink(metrics::S2);
    let (_, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    let root = app.location;
    if root == NONE || root as usize >= t.len() {
        return;
    }
    let stale = match &app.treemap {
        Some(l) => l.root != root || l.area != area || app.treemap_stale,
        None => true,
    };
    if stale {
        app.treemap = Some(treemap::build(&t, root, area, app.logical, p, app.mode()));
        app.treemap_stale = false;
    }
    let layout = app.treemap.as_ref().unwrap();
    let painter = ui.painter_at(area.expand(2.0));
    painter.add(egui::Shape::Mesh(layout.mesh.clone()));

    let hovered = resp.hover_pos().and_then(|pos| treemap::hit(layout, pos));
    if let Some(h) = hovered
        && let Some(c) = layout.cells.iter().find(|c| c.node == h)
    {
        painter.rect_stroke(
            c.rect,
            0.0,
            Stroke::new(1.0_f32, p.text_strong),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(sel) = app.selected {
        // The selection may be a folder the map shows as many cells: outline
        // the area its cells cover.
        let mut bounds = Rect::NOTHING;
        for c in &layout.cells {
            if t.is_within(c.node, sel) {
                bounds = bounds.union(c.rect);
            }
        }
        if bounds.is_positive() && sel != root {
            painter.rect_stroke(
                bounds,
                0.0,
                Stroke::new(2.0_f32, p.accent),
                egui::StrokeKind::Inside,
            );
        }
    }
    let mut action = None;
    if let Some(h) = hovered {
        let tip = format!(
            "{}\n{}",
            t.path(h).display(),
            format_size(t.size(h, app.logical), app.units)
        );
        let resp = resp.clone().on_hover_text_at_pointer(tip);
        if resp.clicked() {
            app.selected = Some(h);
        }
        if resp.double_clicked() {
            // Zoom into the folder under the pointer, one level below here.
            let mut z = h;
            while t.parent(z) != root && t.parent(z) != NONE {
                z = t.parent(z);
            }
            if t.is_dir(z) {
                action = Some((z, Action::ZoomIn));
            }
        }
        resp.context_menu(|ui| {
            if let Some(a) = context_menu(ui, app, h) {
                action = Some((h, a));
            }
        });
    }
    drop(t);
    if let Some((n, a)) = action {
        app.act(n, a);
    }
}

pub fn types_view(ui: &mut egui::Ui, p: Palette, app: &mut App) {
    let Some(r) = app.reports_ready() else {
        return waiting(ui, p, app);
    };
    let total: u64 = r.types.iter().map(|s| s.bytes).sum::<u64>().max(1);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(metrics::S3);
            section_title(ui, p, "By kind");
            for &(kind, bytes, count) in &r.kinds {
                if count == 0 {
                    continue;
                }
                stat_row(
                    ui,
                    p,
                    Some(treemap::kind_colour(kind, app.mode())),
                    kind.label(),
                    bytes,
                    count,
                    total,
                    app,
                );
            }
            ui.add_space(metrics::S4);
            section_title(ui, p, "By extension");
            for s in r.types.iter().take(200) {
                let label = if s.ext.is_empty() {
                    "(no extension)".to_string()
                } else {
                    format!(".{}", s.ext)
                };
                stat_row(
                    ui,
                    p,
                    Some(treemap::kind_colour(s.kind, app.mode())),
                    &label,
                    s.bytes,
                    s.count,
                    total,
                    app,
                );
            }
            if r.types.len() > 200 {
                ui.add_space(metrics::S2);
                ui.horizontal(|ui| {
                    ui.add_space(metrics::PAD_STRIP);
                    ui.label(
                        egui::RichText::new(format!(
                            "and {} more extensions",
                            format_count(r.types.len() as u64 - 200)
                        ))
                        .size(text::SMALL)
                        .color(p.text_dim),
                    );
                });
            }
            ui.add_space(metrics::S4);
        });
}

pub fn largest_view(ui: &mut egui::Ui, p: Palette, app: &mut App) {
    let Some(r) = app.reports_ready() else {
        return waiting(ui, p, app);
    };
    let Some(scan) = app.scan.clone() else { return };
    let t = scan.tree();
    let largest = r.largest.clone();
    let mut select = None;
    let mut action = None;
    let width = ui.available_width();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, metrics::ROW, largest.len(), |ui, range| {
            for &n in &largest[range] {
                if t.has(n, flag::REMOVED) {
                    continue;
                }
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(width, metrics::ROW), Sense::click());
                let selected = app.selected == Some(n);
                if selected {
                    ui.painter().rect_filled(rect, 0.0, p.control);
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            rect.left_top(),
                            vec2(metrics::NAV_MARK_W, rect.height()),
                        ),
                        0.0,
                        p.accent,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, p.control_hover);
                }
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    rect.bottom(),
                    Stroke::new(metrics::HAIRLINE, p.line_soft),
                );
                let painter = ui.painter();
                let cy = rect.center().y;
                painter.text(
                    pos2(rect.left() + metrics::PAD_STRIP + 80.0, cy),
                    Align2::RIGHT_CENTER,
                    format_size(t.size(n, app.logical), app.units),
                    FontId::monospace(text::TINY),
                    p.text,
                );
                let sw = Rect::from_center_size(
                    pos2(rect.left() + metrics::PAD_STRIP + 96.0, cy),
                    Vec2::splat(8.0),
                );
                painter.rect_filled(
                    sw,
                    metrics::RADIUS_TIGHT,
                    treemap::kind_colour(heft_scan::kinds::kind_of(t.name_bytes(n)), app.mode()),
                );
                let name = t.display_name(n);
                let g = painter.layout_no_wrap(
                    name,
                    if selected {
                        theme::strong(text::BODY)
                    } else {
                        FontId::proportional(text::BODY)
                    },
                    p.text_strong,
                );
                let nx = rect.left() + metrics::PAD_STRIP + 110.0;
                let gw = g.size().x;
                painter.galley(pos2(nx, cy - g.size().y / 2.0), g, p.text_strong);
                let parent = t.path(t.parent(n)).display().to_string();
                painter.text(
                    pos2(nx + gw + metrics::S3, cy),
                    Align2::LEFT_CENTER,
                    parent,
                    FontId::proportional(text::SMALL),
                    p.text_dim,
                );
                if resp.clicked() {
                    select = Some(n);
                }
                resp.context_menu(|ui| {
                    if let Some(a) = context_menu(ui, app, n) {
                        action = Some((n, a));
                    }
                });
            }
        });
    drop(t);
    if let Some(n) = select {
        app.selected = Some(n);
    }
    if let Some((n, a)) = action {
        app.act(n, a);
    }
}

pub fn age_view(ui: &mut egui::Ui, p: Palette, app: &mut App) {
    let Some(r) = app.reports_ready() else {
        return waiting(ui, p, app);
    };
    let total: u64 = r.age.iter().map(|a| a.0).sum::<u64>().max(1);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(metrics::S3);
        section_title(ui, p, "By last modification");
        for (i, &(bytes, count)) in r.age.iter().enumerate() {
            let label = AGE_BUCKETS.get(i).map(|b| b.0).unwrap_or("Unknown");
            if i == AGE_BUCKETS.len() && count == 0 {
                continue;
            }
            stat_row(ui, p, None, label, bytes, count, total, app);
        }
        ui.add_space(metrics::S3);
        ui.horizontal(|ui| {
            ui.add_space(metrics::PAD_STRIP);
            ui.label(
                egui::RichText::new("Files that nobody has changed in years are often the safest place to start.")
                    .size(text::SMALL)
                    .color(p.text_dim),
            );
        });
    });
}

fn section_title(ui: &mut egui::Ui, p: Palette, title: &str) {
    ui.horizontal(|ui| {
        ui.add_space(metrics::PAD_STRIP);
        ui.label(
            egui::RichText::new(title)
                .font(theme::strong(text::HEADING))
                .color(p.text_strong),
        );
    });
    ui.add_space(metrics::S2);
}

#[allow(clippy::too_many_arguments)]
fn stat_row(
    ui: &mut egui::Ui,
    p: Palette,
    swatch: Option<Color32>,
    label: &str,
    bytes: u64,
    count: u64,
    total: u64,
    app: &App,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, metrics::ROW), Sense::hover());
    let painter = ui.painter();
    let cy = rect.center().y;
    let mut x = rect.left() + metrics::PAD_STRIP;
    if let Some(c) = swatch {
        painter.rect_filled(
            Rect::from_center_size(pos2(x + 4.0, cy), Vec2::splat(8.0)),
            metrics::RADIUS_TIGHT,
            c,
        );
        x += 16.0;
    }
    painter.text(
        pos2(x, cy),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(text::BODY),
        p.text,
    );
    let bar_left = rect.left() + 260.0;
    let bar_w = (width - 260.0 - 300.0).max(80.0);
    rail_at(
        painter,
        p,
        Rect::from_min_size(pos2(bar_left, cy - 2.0), vec2(bar_w, 4.0)),
        Some(bytes as f32 / total as f32),
        p.accent_dim,
    );
    let fig = FontId::monospace(text::TINY);
    let mut rx = bar_left + bar_w + 96.0;
    painter.text(
        pos2(rx, cy),
        Align2::RIGHT_CENTER,
        format_size(bytes, app.units),
        fig.clone(),
        p.text,
    );
    rx += 64.0;
    painter.text(
        pos2(rx, cy),
        Align2::RIGHT_CENTER,
        format!("{:.1} %", bytes as f64 * 100.0 / total as f64),
        fig.clone(),
        p.text_muted,
    );
    rx += 120.0;
    painter.text(
        pos2(rx, cy),
        Align2::RIGHT_CENTER,
        format!("{} files", format_count(count)),
        fig,
        p.text_muted,
    );
}

fn no_scan(ui: &mut egui::Ui, p: Palette) {
    empty_state(
        ui,
        p,
        "Nothing scanned yet",
        "Choose a disk or a folder on the left to see what is taking the space.",
    );
}

fn waiting(ui: &mut egui::Ui, p: Palette, app: &App) {
    if app.scan.is_none() {
        return no_scan(ui, p);
    }
    empty_state(
        ui,
        p,
        "Counting",
        "This view fills in once the folder has been read.",
    );
}

/// What a report pass produced, and for which folder.
pub struct Reports {
    pub root: NodeId,
    pub generation: u64,
    pub logical: bool,
    pub types: Vec<TypeStat>,
    pub kinds: Vec<(FileKind, u64, u64)>,
    pub largest: Vec<NodeId>,
    pub age: Vec<(u64, u64)>,
}

pub fn compute_reports(t: &Tree, root: NodeId, generation: u64, logical: bool) -> Reports {
    let types = report::by_extension(t, root, logical);
    let kinds = report::by_kind(&types);
    let largest = report::largest_files(t, root, 500, logical);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let age = report::by_age(t, root, now, logical);
    Reports {
        root,
        generation,
        logical,
        types,
        kinds,
        largest,
        age,
    }
}

/// Seconds since 1970 to `YYYY-MM-DD`, in UTC.
pub fn date(secs: u32) -> String {
    let days = secs as i64 / 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn view_label(v: View) -> &'static str {
    match v {
        View::Tree => "Folders",
        View::Treemap => "Treemap",
        View::Types => "File types",
        View::Largest => "Largest files",
        View::Age => "Age",
    }
}

pub fn view_icon(v: View) -> Icon {
    match v {
        View::Tree => Icon::Tree,
        View::Treemap => Icon::Treemap,
        View::Types => Icon::Types,
        View::Largest => Icon::File,
        View::Age => Icon::Age,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates() {
        assert_eq!(super::date(0), "1970-01-01");
        assert_eq!(super::date(1_700_000_000), "2023-11-14");
        assert_eq!(super::date(951_782_400), "2000-02-29");
    }
}
