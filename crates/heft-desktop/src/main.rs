//! `heft` with no arguments opens the window; `heft <subcommand>` is the
//! text mode, the same engine without one; `heft <path>` opens the window on
//! that path.

// A release build declares the **windows** subsystem, so that opening Heft
// from the Start menu, from Explorer or from the installer's "Start Heft" does
// not put a console window behind it (Muster 0.0.3 shipped without this and
// did exactly that). A GUI-subsystem process does not inherit the console it
// was started from either, so `attach_parent_console` is the other half: it
// takes the terminal's console back, or the text mode would print nothing.
// A debug build keeps the console subsystem, because a panic message with
// nowhere to go is worse than a console window nobody minds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;

fn main() {
    // Before the first write to either stream: that is when the standard
    // library resolves the handle.
    attach_parent_console();
    env_logger::Builder::from_env(
        env_logger::Env::default().filter_or("RUST_LOG", "warn,wgpu_hal=error,wgpu_core=error"),
    )
    .init();

    // This executable is also its own installer, twice over, and both are read
    // before anything else because neither is a Heft command.
    //
    // `--install-update` is the helper an update spawns: a running executable
    // cannot replace itself, so the process that puts the package in place
    // cannot be this one. `heft-setup.exe` is this same binary with an MSI
    // appended to it, and it arrives with **no arguments at all**, because it
    // is double-clicked. So the payload on the end of the file is what tells
    // setup from Heft; asking the command line alone would leave the installer
    // unreachable (Muster 0.0.8). See `heft_app::update::installer` and
    // `update::payload`.
    let carries_payload =
        std::env::current_exe().is_ok_and(|exe| heft_app::update::payload::carried_by(&exe));
    if let Some(job) = heft_app::update::installer::job(std::env::args(), carries_payload) {
        if let Err(e) = heft_app::update::installwin::show(job) {
            eprintln!("heft: the installer could not open a window: {e}");
            std::process::exit(1);
        }
        return;
    }

    // A Windows update leaves the binary it displaced beside the new one,
    // because a running executable cannot be deleted. This is the first moment
    // it can go.
    heft_app::update::sweep_previous_binary();

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

/// Take the console Heft was launched from, where there is one.
///
/// `AttachConsole(ATTACH_PARENT_PROCESS)` asks for the parent's console and
/// fails harmlessly where there is none (started from Explorer, the Start menu
/// or the installer), which is exactly the case the subsystem change exists
/// for. The known wart: a GUI-subsystem process does not hold the shell's
/// prompt, so text-mode output arrives after the prompt has come back.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    // SAFETY: no pointer arguments, and a documented failure return for "there
    // is no console to attach to", which is the ordinary case and is ignored.
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(
            windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }
}

/// Every other build already has whatever console it is going to get.
#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}
