//! `heft` with no arguments opens the window; `heft <subcommand>` is the
//! text mode, the same engine without one.

mod cli;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    if let Err(e) = heft_app::run() {
        eprintln!("Heft could not open its window: {e}");
        std::process::exit(1);
    }
}
