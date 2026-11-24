//! The right-hand details panel beside the treemap: what the selected cell
//! is, where it lives, how big it is and what can be done with it.
//!
//! With nothing selected it describes the folder the map is showing, so the
//! panel is never blank.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{self, Palette, metrics, text};
use crate::treemap;
use crate::views::{self, Action};
use crate::widgets::{self, Kind};
use egui::{Align, Layout, RichText, Sense, Stroke, Vec2};
use heft_scan::NONE;
use heft_scan::kinds;
use heft_scan::remove;
use heft_scan::tree::flag;
use heft_scan::units::{format_count, format_size};

pub fn panel(ctx: &egui::Context, p: Palette, app: &mut App) {
    egui::SidePanel::right("details")
        .exact_width(metrics::PANEL)
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(p.dock)
                .inner_margin(egui::Margin::same(metrics::PAD_PANEL as i8)),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            let r = ui.max_rect().expand(metrics::PAD_PANEL);
            ui.painter().vline(
                r.left(),
                r.top()..=r.bottom(),
                Stroke::new(metrics::HAIRLINE, p.line),
            );
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| body(ui, p, app));
        });
}

fn body(ui: &mut egui::Ui, p: Palette, app: &mut App) {
    let Some(scan) = app.scan.clone() else {
        return;
    };
    let t = scan.tree();
    let here = app.location;
    let n = match app.selected {
        Some(s) if (s as usize) < t.len() && !t.has(s, flag::REMOVED) => s,
        _ => here,
    };
    if n == NONE || n as usize >= t.len() {
        return;
    }
    let is_dir = t.is_dir(n);
    let flags = t.flags(n);
    let logical = app.logical;

    label(
        ui,
        p,
        if app.selected.is_some() && n != here {
            "Selected"
        } else {
            "This folder"
        },
    );

    // Icon and name.
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(metrics::ICON), Sense::hover());
        let icon = if t.parent(n) == NONE {
            Icon::Drive
        } else if is_dir {
            Icon::Folder
        } else {
            Icon::File
        };
        icons::draw(ui.painter(), rect, icon, p.text_muted);
        ui.add(
            egui::Label::new(
                RichText::new(t.display_name(n))
                    .font(theme::strong(text::HEADING))
                    .color(p.text_strong),
            )
            .wrap(),
        );
    });
    ui.add_space(metrics::S1);
    let path = t.path(n);
    let location = t.path(t.parent(n));
    let shown = if t.parent(n) == NONE {
        &path
    } else {
        &location
    };
    ui.add(
        egui::Label::new(
            RichText::new(shown.display().to_string())
                .size(text::SMALL)
                .color(p.text_dim),
        )
        .wrap(),
    );

    // Size.
    label(ui, p, "Size");
    let size = t.size(n, logical);
    ui.label(
        RichText::new(format_size(size, app.units))
            .font(theme::strong(text::PAGE))
            .color(p.text_strong),
    );
    ui.add_space(metrics::S1);
    row(ui, p, "On disk", &format_size(t.disk(n), app.units));
    row(ui, p, "Logical", &format_size(t.logical(n), app.units));
    if !t.has(n, flag::COMPLETE) && is_dir && !t.has(n, flag::UNREADABLE) {
        note(ui, p, "Still being scanned. The size will grow.");
    }

    // Share of the parent and of the folder the map shows.
    let parent = t.parent(n);
    if parent != NONE {
        share(
            ui,
            p,
            &format!("Of {}", short_name(&t, parent)),
            size,
            t.size(parent, logical),
        );
    }
    if here != NONE && here != n && here != parent && t.is_within(n, here) {
        share(
            ui,
            p,
            &format!("Of {}", short_name(&t, here)),
            size,
            t.size(here, logical),
        );
    }

    // What it is.
    label(ui, p, "Details");
    if is_dir {
        row(ui, p, "Files", &format_count(t.files(n) as u64));
        row(
            ui,
            p,
            "Items directly inside",
            &format_count(t.children(n).len() as u64),
        );
    } else {
        let kind = kinds::kind_of(t.name_bytes(n));
        let ext = kinds::extension(t.name_bytes(n))
            .map(|e| format!(".{e}"))
            .unwrap_or_else(|| "no extension".into());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Type").size(text::BODY).color(p.text_muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} ({ext})", kind.label()))
                        .size(text::BODY)
                        .color(p.text),
                );
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    metrics::RADIUS_TIGHT,
                    treemap::kind_colour(kind, app.mode()),
                );
            });
        });
    }
    if t.mtime(n) != 0 {
        row(ui, p, "Modified", &views::date(t.mtime(n)));
    }
    if flags & flag::UNREADABLE != 0 {
        note(
            ui,
            p,
            "Could not be read, so its size is unknown, not zero.",
        );
    }
    if flags & flag::LINK != 0 {
        note(
            ui,
            p,
            "A link. Heft does not follow links, so what it points to is not counted here.",
        );
    }
    if flags & flag::HARDLINK_DUP != 0 {
        note(
            ui,
            p,
            "A hard link to a file already counted at another path. Not counted twice.",
        );
    }
    if flags & flag::OTHER_FS != 0 {
        note(
            ui,
            p,
            "Another drive or filesystem is mounted here. It was not scanned.",
        );
    }

    // Actions.
    ui.add_space(metrics::S4);
    let mut action = None;
    if is_dir
        && n != here
        && widgets::button_with(
            ui,
            p,
            Some(Icon::Treemap),
            "Open in treemap",
            Kind::Secondary,
            true,
        )
        .clicked()
    {
        action = Some(Action::ZoomIn);
    }
    ui.add_space(metrics::S1);
    if widgets::button_with(
        ui,
        p,
        Some(Icon::Folder),
        views::show_label(),
        Kind::Secondary,
        true,
    )
    .clicked()
    {
        action = Some(Action::Show);
    }
    ui.add_space(metrics::S1);
    if widgets::button(ui, p, "Copy path", Kind::Secondary, true).clicked() {
        ui.ctx().copy_text(path.display().to_string());
    }
    ui.add_space(metrics::S1);
    let refusal = remove::refusal_with(&scan, &t, n);
    let r = widgets::button_with(
        ui,
        p,
        Some(Icon::Trash),
        App::trash_label(),
        Kind::Outlined,
        refusal.is_none(),
    );
    let r = match &refusal {
        Some(why) => r.on_disabled_hover_text(why),
        None => r,
    };
    if r.clicked() {
        action = Some(Action::Remove);
    }
    drop(t);
    if let Some(a) = action {
        app.act(n, a);
    }
}

fn label(ui: &mut egui::Ui, p: Palette, s: &str) {
    ui.add_space(metrics::S3);
    ui.label(
        RichText::new(s.to_uppercase())
            .size(text::EYEBROW)
            .color(p.text_dim),
    );
    ui.add_space(metrics::S1);
}

fn row(ui: &mut egui::Ui, p: Palette, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(k).size(text::BODY).color(p.text_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(v).size(text::BODY).color(p.text));
        });
    });
}

fn share(ui: &mut egui::Ui, p: Palette, k: &str, part: u64, whole: u64) {
    let f = if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    };
    row(
        ui,
        p,
        &widgets::ellipsize(k, 26),
        &format!("{:.1} %", f * 100.0),
    );
    widgets::rail(ui, p, ui.available_width(), Some(f as f32));
    ui.add_space(metrics::S1);
}

fn note(ui: &mut egui::Ui, p: Palette, s: &str) {
    ui.add_space(metrics::S1);
    ui.add(egui::Label::new(RichText::new(s).size(text::SMALL).color(p.text_dim)).wrap());
}

/// A folder's own name; for a scan root (whose name is its whole path) the
/// last part of that path.
fn short_name(t: &heft_scan::Tree, n: heft_scan::NodeId) -> String {
    if t.parent(n) == NONE {
        let path = t.path(n);
        if let Some(last) = path.file_name() {
            return last.to_string_lossy().into_owned();
        }
        return path.display().to_string();
    }
    t.display_name(n)
}
