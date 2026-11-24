//! The painted controls, Muster's (and so the style guide's §7) rather than
//! egui's stock ones.

use crate::icons::{self, Icon};
use crate::theme::{self, Palette, metrics, text};
use egui::{Color32, FontId, Rect, RichText, Sense, Stroke, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Outlined,
    Ghost,
}

fn skin(p: Palette, kind: Kind, hovered: bool) -> (Option<Color32>, Option<Color32>, Color32) {
    match kind {
        Kind::Primary => (Some(p.accent), None, p.accent_ink),
        Kind::Secondary => (
            Some(if hovered { p.control_hover } else { p.control }),
            None,
            p.text,
        ),
        Kind::Outlined => (
            None,
            Some(if hovered { p.line_dashed } else { p.line }),
            p.text,
        ),
        Kind::Ghost => (
            None,
            None,
            if hovered { p.text_strong } else { p.text_muted },
        ),
    }
}

/// A toggle, §7.8: 34 x 18, knob left when off and right when on (Muster's).
pub fn toggle(ui: &mut egui::Ui, p: Palette, on: &mut bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 18.0), Sense::click());
    let changed = response.clicked();
    if changed {
        *on = !*on;
    }
    ui.painter()
        .rect_filled(rect, 9.0, if *on { p.accent } else { p.control });
    if !*on {
        ui.painter().rect_stroke(
            rect,
            9.0,
            Stroke::new(metrics::HAIRLINE, p.line),
            egui::StrokeKind::Inside,
        );
    }
    let x = if *on {
        rect.right() - 9.0
    } else {
        rect.left() + 9.0
    };
    ui.painter().circle_filled(
        pos2(x, rect.center().y),
        6.0,
        if *on { p.accent_ink } else { p.knob },
    );
    changed
}

/// A §7.6 button: 26 px, radius 5, 11.5 px text. Disabled is 45 % across the
/// whole control, and the caller adds a tooltip saying why.
pub fn button(
    ui: &mut egui::Ui,
    p: Palette,
    label: &str,
    kind: Kind,
    enabled: bool,
) -> egui::Response {
    button_with(ui, p, None, label, kind, enabled)
}

pub fn button_with(
    ui: &mut egui::Ui,
    p: Palette,
    icon: Option<Icon>,
    label: &str,
    kind: Kind,
    enabled: bool,
) -> egui::Response {
    let font = match kind {
        Kind::Primary => theme::strong(text::CONTROL),
        _ => FontId::proportional(text::CONTROL),
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let icon_w = if icon.is_some() {
        metrics::ICON + metrics::S1 + 2.0
    } else {
        0.0
    };
    let size = vec2(
        galley.size().x + icon_w + metrics::PAD_PANEL * 2.0,
        metrics::BUTTON,
    );
    let (rect, response) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let (fill, border, ink) = skin(p, kind, enabled && response.hovered());
    let dim = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, metrics::RADIUS, dim(fill));
    }
    if let Some(border) = border {
        ui.painter().rect_stroke(
            rect,
            metrics::RADIUS,
            Stroke::new(metrics::HAIRLINE, dim(border)),
            egui::StrokeKind::Inside,
        );
    }
    let mut x = rect.left() + metrics::PAD_PANEL;
    if let Some(icon) = icon {
        let r = Rect::from_center_size(
            pos2(x + metrics::ICON / 2.0, rect.center().y),
            Vec2::splat(metrics::ICON - 2.0),
        );
        icons::draw(ui.painter(), r, icon, dim(ink));
        x += icon_w;
    }
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        dim(ink),
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A square ghost button holding one icon. Always give it a tooltip.
pub fn icon_button(
    ui: &mut egui::Ui,
    p: Palette,
    icon: Icon,
    tip: &str,
    enabled: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(metrics::BUTTON),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hovered = enabled && response.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, metrics::RADIUS, p.control_hover);
    }
    let ink = if !enabled {
        p.text_dim.gamma_multiply(0.45)
    } else if hovered {
        p.text_strong
    } else {
        p.text_muted
    };
    icons::draw(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(metrics::ICON)),
        icon,
        ink,
    );
    let response = response.on_hover_text(tip);
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// §7.9 segmented control. Returns the index chosen this frame, if any.
pub fn segmented(ui: &mut egui::Ui, p: Palette, labels: &[&str], selected: usize) -> Option<usize> {
    let font = FontId::proportional(text::CONTROL);
    let pad = metrics::S3;
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap(l.to_string(), font.clone(), Color32::PLACEHOLDER)
                .size()
                .x
                + pad * 2.0
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, metrics::BUTTON), Sense::hover());
    ui.painter().rect_filled(rect, metrics::RADIUS, p.field);
    ui.painter().rect_stroke(
        rect,
        metrics::RADIUS,
        Stroke::new(metrics::HAIRLINE, p.line),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 2.0;
    let mut chosen = None;
    for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
        let seg = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(*w, rect.height() - 4.0));
        let r = ui.interact(seg, ui.id().with(("seg", label, i)), Sense::click());
        let on = i == selected;
        if on {
            ui.painter()
                .rect_filled(seg, metrics::RADIUS_TIGHT, p.control);
        } else if r.hovered() {
            ui.painter()
                .rect_filled(seg, metrics::RADIUS_TIGHT, p.control_hover);
        }
        ui.painter().text(
            seg.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            if on {
                theme::strong(text::CONTROL)
            } else {
                font.clone()
            },
            if on { p.text_strong } else { p.text_muted },
        );
        if r.clicked() && !on {
            chosen = Some(i);
        }
        x += w;
    }
    chosen
}

const NAV_GAP: f32 = 10.0;

/// A navigation row: neutral fill, strong text and a 3 px accent bar when
/// selected. Never an accent background. `note` is a right-aligned figure.
pub fn nav_row(
    ui: &mut egui::Ui,
    p: Palette,
    icon: Icon,
    label: &str,
    note: Option<&str>,
    selected: bool,
) -> egui::Response {
    let size = vec2(ui.available_width(), metrics::NAV_ROW);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if selected {
        ui.painter().rect_filled(rect, metrics::RADIUS, p.control);
        let mark = Rect::from_min_size(
            rect.left_top() + vec2(0.0, metrics::S1),
            vec2(metrics::NAV_MARK_W, rect.height() - metrics::S2),
        );
        ui.painter()
            .rect_filled(mark, metrics::RADIUS_TIGHT, p.accent);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, metrics::RADIUS, p.control_hover);
    }
    let ink = if selected {
        p.text_strong
    } else {
        p.text_muted
    };
    let glyph = Rect::from_center_size(
        rect.left_center() + vec2(metrics::PAD_PANEL + metrics::ICON / 2.0, 0.0),
        Vec2::splat(metrics::ICON),
    );
    icons::draw(ui.painter(), glyph, icon, ink);
    let right = if let Some(note) = note {
        let g = ui.painter().layout_no_wrap(
            note.to_string(),
            FontId::monospace(text::TINY),
            p.text_dim,
        );
        let w = g.size().x;
        ui.painter().galley(
            pos2(
                rect.right() - metrics::S2 - w,
                rect.center().y - g.size().y / 2.0,
            ),
            g,
            p.text_dim,
        );
        rect.right() - metrics::S2 - w - metrics::S2
    } else {
        rect.right() - metrics::S2
    };
    let font = FontId::proportional(text::CONTROL);
    let galley = ui.painter().layout(
        label.to_string(),
        font,
        ink,
        (right - glyph.right() - NAV_GAP).max(10.0),
    );
    let mut job_galley = galley;
    if job_galley.rows.len() > 1 {
        job_galley = ui.painter().layout_no_wrap(
            ellipsize(label, 28),
            FontId::proportional(text::CONTROL),
            ink,
        );
    }
    ui.painter().galley(
        pos2(
            glyph.right() + NAV_GAP,
            rect.center().y - job_galley.size().y / 2.0,
        ),
        job_galley,
        ink,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A short label cut at `max` characters with an ellipsis.
pub fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// An eyebrow: the small label over a group of rows.
pub fn eyebrow(ui: &mut egui::Ui, p: Palette, label: &str) {
    ui.add_space(metrics::S3);
    ui.horizontal(|ui| {
        ui.add_space(metrics::PAD_PANEL);
        ui.label(
            RichText::new(label.to_uppercase())
                .size(text::EYEBROW)
                .color(p.text_dim),
        );
    });
    ui.add_space(metrics::S1);
}

/// A 3 px rail, filled in the accent to `fraction`. `None` draws the empty
/// track and nothing in it: the bar never animates over an unknown.
pub fn rail(ui: &mut egui::Ui, p: Palette, width: f32, fraction: Option<f32>) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, metrics::RAIL), Sense::hover());
    rail_at(ui.painter(), p, rect, fraction, p.accent);
}

pub fn rail_at(
    painter: &egui::Painter,
    p: Palette,
    rect: Rect,
    fraction: Option<f32>,
    fill: Color32,
) {
    painter.rect_filled(rect, metrics::RAIL / 2.0, p.rail);
    if let Some(f) = fraction {
        let filled = Rect::from_min_size(
            rect.left_top(),
            vec2(rect.width() * f.clamp(0.0, 1.0), rect.height()),
        );
        painter.rect_filled(filled, metrics::RAIL / 2.0, fill);
    }
}

pub fn hairline_bottom(ui: &egui::Ui, p: Palette) {
    let r = ui.max_rect();
    ui.painter().hline(
        r.left()..=r.right(),
        r.bottom(),
        Stroke::new(metrics::HAIRLINE, p.line),
    );
}
pub fn hairline_top(ui: &egui::Ui, p: Palette) {
    let r = ui.max_rect();
    ui.painter().hline(
        r.left()..=r.right(),
        r.top(),
        Stroke::new(metrics::HAIRLINE, p.line),
    );
}
pub fn hairline_right(ui: &egui::Ui, p: Palette) {
    let r = ui.max_rect();
    ui.painter().vline(
        r.right(),
        r.top()..=r.bottom(),
        Stroke::new(metrics::HAIRLINE, p.line),
    );
}
pub fn hairline_across(ui: &egui::Ui, p: Palette, y: f32) {
    let r = ui.max_rect();
    ui.painter().hline(
        r.left()..=r.right(),
        y,
        Stroke::new(metrics::HAIRLINE, p.line_soft),
    );
}

pub fn empty_state(ui: &mut egui::Ui, p: Palette, message: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(metrics::S4 * 4.0);
        ui.label(
            RichText::new(message)
                .font(theme::strong(text::HEADING))
                .color(p.text),
        );
        ui.add_space(metrics::S2);
        ui.label(RichText::new(detail).size(text::BODY).color(p.text_dim));
    });
}
