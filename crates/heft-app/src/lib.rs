//! Heft's window.
//!
//! The interface and nothing else: every number on screen comes out of
//! `heft-scan` through the same API the text mode uses. The token table in
//! [`theme`] is `../Design-Principles/tokens.css` transcribed (via Muster's
//! port of Umber's), and the widgets in [`widgets`] are Muster's, so the two
//! applications are rooms in the same house.

pub mod app;
pub mod art;
pub mod details;
pub mod icons;
pub mod lucide;
pub mod theme;
pub mod treemap;
pub mod update;
pub mod updatedlg;
pub mod views;
pub mod widgets;

/// The application id, which is also the desktop entry's name (Wayland reads
/// the taskbar icon out of `<APP_ID>.desktop`). `packaging/check.sh` will
/// keep the two in step once packaging exists.
pub const APP_ID: &str = "io.github.spillebulle.heft";

pub const WINDOW: [f32; 2] = [1280.0, 780.0];

/// What the window opens on: paths to scan at once, and a view to show.
#[derive(Default)]
pub struct Start {
    pub roots: Vec<std::path::PathBuf>,
    pub view: Option<String>,
}

pub fn run(start: Start) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(WINDOW)
            .with_min_inner_size([820.0, 480.0])
            .with_title("Heft")
            // The mark, rasterised from the palette so the taskbar cannot show
            // a stale accent.
            .with_icon(art::window_icon())
            .with_app_id(APP_ID),
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, start)))),
    )
    .map_err(|e| e.to_string())
}

/// Archivo, bundled, at the two weights §4 allows (static instances cut from
/// the variable font, because egui's rasteriser ignores variation axes).
pub(crate) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "archivo",
            include_bytes!("../assets/Archivo-Regular.ttf").as_slice(),
        ),
        (
            theme::STRONG,
            include_bytes!("../assets/Archivo-SemiBold.ttf").as_slice(),
        ),
    ] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "archivo".into());
    fonts.families.insert(
        egui::FontFamily::Name(theme::STRONG.into()),
        vec![theme::STRONG.to_string()],
    );
    ctx.set_fonts(fonts);
}

/// Pushes the palette into egui's own visuals, for what egui draws itself.
pub(crate) fn apply_visuals(ctx: &egui::Context, p: theme::Palette, mode: theme::Mode) {
    use egui::Stroke;
    use theme::metrics;
    let mut v = match mode {
        theme::Mode::Dark => egui::Visuals::dark(),
        theme::Mode::Light => egui::Visuals::light(),
    };
    v.panel_fill = p.window;
    v.window_fill = p.popover;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.dock;
    v.widgets.noninteractive.bg_stroke = Stroke::new(metrics::HAIRLINE, p.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(metrics::HAIRLINE, p.text);
    v.window_stroke = Stroke::new(metrics::HAIRLINE, p.line_popover);
    v.window_shadow = theme::shadow::modal(mode);
    v.popup_shadow = theme::shadow::menu(mode);
    v.selection.bg_fill = p.accent_ring;
    v.selection.stroke = Stroke::new(metrics::HAIRLINE, p.text_strong);
    v.window_corner_radius = egui::CornerRadius::same(metrics::RADIUS_MODAL as u8);
    v.menu_corner_radius = egui::CornerRadius::same(metrics::RADIUS_CARD as u8);
    v.widgets.hovered.weak_bg_fill = p.control_hover;
    v.widgets.inactive.weak_bg_fill = p.control;
    v.widgets.inactive.fg_stroke = Stroke::new(metrics::HAIRLINE, p.text);
    v.widgets.hovered.fg_stroke = Stroke::new(metrics::HAIRLINE, p.text_strong);
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(metrics::S2, metrics::S1);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.button_padding = egui::vec2(metrics::S2, metrics::S1);
    style.interaction.tooltip_delay = 0.35;
    for (ts, size) in [
        (egui::TextStyle::Body, theme::text::BODY),
        (egui::TextStyle::Button, theme::text::CONTROL),
        (egui::TextStyle::Small, theme::text::SMALL),
        (egui::TextStyle::Monospace, theme::text::TINY),
    ] {
        if let Some(f) = style.text_styles.get_mut(&ts) {
            f.size = size;
        }
    }
    ctx.set_style(style);
}
