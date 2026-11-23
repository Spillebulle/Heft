//! `heft` with no arguments opens the window; `heft <subcommand>` is the
//! text mode, the same engine without one.

mod cli;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    let mut roots = Vec::new();
    let mut view = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--view" {
            view = it.next().cloned();
        } else {
            roots.push(std::path::PathBuf::from(a));
        }
    }
    if let Err(e) = heft_app::run(heft_app::Start { roots, view }) {
        eprintln!("Heft could not open its window: {e}");
        std::process::exit(1);
    }
}
