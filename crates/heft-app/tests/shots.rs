//! Renders the real window headlessly and writes PNGs to `target/shots/`.
//! Not a pass/fail test: run it, then look at the pictures.
//!   HEFT_SHOT_ROOT=<folder> cargo test -p heft-app --test shots -- --ignored --nocapture

use egui_kittest::Harness;
use std::time::{Duration, Instant};

fn shoot(view: &str, dark: bool) {
    let root =
        std::env::var("HEFT_SHOT_ROOT").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string());
    let start = heft_app::Start {
        roots: vec![root.into()],
        view: Some(view.into()),
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(heft_app::WINDOW[0], heft_app::WINDOW[1]))
        .build_eframe(|cc| {
            cc.egui_ctx.set_theme(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            heft_app::app::App::new(cc, start)
        });
    harness.state_mut().theme = if dark {
        heft_app::app::ThemePref::Dark
    } else {
        heft_app::app::ThemePref::Light
    };
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(60) {
        harness.run_steps(2);
        if harness
            .state()
            .scan
            .as_ref()
            .is_some_and(|s| s.is_finished())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // A few more frames for the reports thread and the throttle.
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(60));
        harness.run_steps(1);
    }
    let image = harness.render().expect("render");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shots");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{view}-{}.png",
        if dark { "dark" } else { "light" }
    ));
    image.save(&path).unwrap();
    println!("wrote {}", path.display());
}

#[test]
#[ignore]
fn shots() {
    for v in ["folders", "treemap", "types", "largest", "age"] {
        shoot(v, true);
    }
    shoot("folders", false);
    shoot("treemap", false);
}
