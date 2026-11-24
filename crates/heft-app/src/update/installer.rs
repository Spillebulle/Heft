//! Installing a Windows package without showing Windows' installer.
//!
//! An update on the MSI installation used to hand `msiexec` the package and get
//! out of the way, which meant the user watched a Windows installer they had
//! not asked for and then had to start Heft again themselves. This is the
//! replacement: a process of Heft's own, drawing Heft's own window, running
//! `msiexec` silently underneath and starting the new build when it is done.
//!
//! ## Why it has to be a second process at all
//!
//! **A running executable cannot be replaced.** Heft's own `heft.exe` is the
//! file the package is about to overwrite, so Heft has to be gone before the
//! installer's execute sequence reaches it — which is exactly why the old
//! design exited. Something else therefore has to hold the window and start the
//! new copy, and that something cannot be Heft.
//!
//! Nor can it be the package: the MSI's "Start Heft" action is published on
//! the exit dialog's Finish button (`packaging/windows/heft.wxs`), so a silent
//! install has no UI sequence to fire it and nothing would relaunch anything.
//!
//! So: the same executable again, with [`FLAG`]. It shares `theme` and
//! `widgets`, so the box is Heft's rather than a second interface.
//!
//! **From a copy in the temporary directory**, and that is not tidiness. The
//! helper would otherwise *be* a file inside the installation the package is
//! replacing, and the installer would find it in use — scheduling a reboot or
//! killing it mid-window. [`stage_helper`] is the copy.
//!
//! ## What cannot be hidden, and is not
//!
//! The **UAC consent prompt**. Heft installs per-machine, so the package needs
//! elevation, and a consent dialog that an application could suppress would not
//! be a security feature. It is asked for once, by [`elevated_msiexec`], and
//! the window says so before it happens rather than letting it arrive unexplained.
//!
//! ## What is testable here
//!
//! Everything except the two calls that touch Windows. The command line is a
//! pure function of the arguments ([`parse`]), the `msiexec` invocation is a
//! pure function of the paths ([`Command::for_package`]), and the stages the
//! window shows are a plain enum. That is the same division `install::detect`
//! keeps, and for the same reason: nobody can cut a release to test against, so
//! the parts that can be checked without one must be.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The flag that turns this executable into the update installer.
pub const FLAG: &str = "--install-update";

/// The flag that turns it into the *first-run* installer.
///
/// `heft-setup.exe` carries the package on its own end — see
/// [`super::payload`] — so this needs no arguments at all, which matters
/// because it is the one flag a person may type or a shortcut may carry.
pub const SETUP_FLAG: &str = "--install";

/// What the helper was asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// The `.msi` to install.
    pub package: PathBuf,
    /// The process to wait for before touching anything, if one was named.
    ///
    /// Heft's own id. The helper is spawned *before* Heft exits — there is no
    /// other moment it could be — so it has to wait, or the installer meets the
    /// running executable it is replacing.
    pub parent: Option<u32>,
    /// The version being installed, for the window to name. Free text out of
    /// the release API, so it is only ever displayed.
    pub version: String,
    /// Whether this is the first-run installer rather than an update.
    ///
    /// The two do the same thing to the machine and differ in what the window
    /// says and in one behaviour: an update was already asked for and gets on
    /// with it, where setup was double-clicked by somebody who has not yet
    /// agreed to anything and waits to be told to start.
    pub setup: bool,
    /// The installed `heft.exe` to start when the package is in place.
    ///
    /// Carried rather than worked out here, and that is the point: this helper
    /// runs from a **copy in the temporary directory** — see [`stage_helper`] —
    /// so its own `current_exe` is the updater, not Heft. Starting that would
    /// reopen the updater.
    pub target: Option<PathBuf>,
}

impl Job {
    /// The first-run installer's job.
    ///
    /// No package path: setup carries its own on the end of this executable and
    /// `installwin` lifts it out. Separate from [`parse`] so that the two ways
    /// in — the flag, and a payload found with no argument at all — build the
    /// identical job rather than two that have to be kept in step.
    pub fn setup() -> Self {
        Self {
            package: PathBuf::new(),
            parent: None,
            version: String::new(),
            setup: true,
            target: None,
        }
    }
}

/// What this launch is, from the command line **and** whether this executable
/// carries a package.
///
/// A pure function of two readings, for the reason [`super::install::detect`]
/// is one of a `Probe`: the interesting rule is which signal wins, and that is
/// worth testing without a file on disk or a command line to fake. `lib.rs`
/// supplies the readings and does no deciding.
///
/// An argument wins where there is one, because it was asked for deliberately —
/// an update spawns the helper with `--install-update <package>` and must not
/// be diverted by the payload a setup binary happens to be carrying. With no
/// argument, a payload means setup. With neither, this is Heft.
pub fn job<I: IntoIterator<Item = String>>(args: I, carries_payload: bool) -> Option<Job> {
    parse(args).or_else(|| carries_payload.then(Job::setup))
}

/// Read the command line.
///
/// A pure function of the arguments, like
/// [`super::install::detect`]. Returns `None` for a command line that is not
/// this — including [`FLAG`] with nothing usable after it, because a helper
/// with no package to install has nothing to do and being Heft is the better
/// answer than being a window that reports its own arguments.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Option<Job> {
    let mut args = args.into_iter().skip(1);
    while let Some(arg) = args.next() {
        // The first-run installer, asked for explicitly. The package is on the
        // end of this file, so the path is empty and the window reads it.
        //
        // **This is not how setup normally starts**, and the comment here used
        // to imply it was. A setup executable is double-clicked, which passes
        // no command line at all, so this arm never fires for the case it was
        // written for — Heft simply started as the application and the
        // installer was unreachable. `payload::carried_by` is the real signal
        // and `lib.rs` consults it when no argument decides. The flag stays as
        // a way to ask deliberately, and for the tests below.
        if arg == SETUP_FLAG {
            return Some(Job::setup());
        }
        if arg != FLAG {
            continue;
        }
        let package = args.next()?;
        if package.is_empty() {
            return None;
        }
        // The two after it are optional and positional. A missing parent means
        // "do not wait", which is right for a helper started by hand.
        let parent = args.next().and_then(|v| v.parse::<u32>().ok());
        let version = args.next().unwrap_or_default();
        let target = args.next().filter(|v| !v.is_empty()).map(PathBuf::from);
        return Some(Job {
            package: PathBuf::from(package),
            parent,
            version,
            setup: false,
            target,
        });
    }
    None
}

/// The `msiexec` invocation for a package.
///
/// A type rather than a bare `Vec` so the arguments can be read in a test
/// without running anything, which is the only way any of this is checked on a
/// machine that cannot install an MSI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: OsString,
    pub args: Vec<OsString>,
    /// Where the installer's own log goes, so a failure has something to show.
    pub log: PathBuf,
}

impl Command {
    /// Install `package`, silently.
    ///
    /// * `/i` — install, and because `packaging/windows/heft.wxs` keeps one
    ///   `UpgradeCode` for ever, that replaces the installed version rather
    ///   than installing beside it.
    /// * `/qn` — no interface at all. This is the whole point: the user asked
    ///   Heft for an update, not for a Windows installer.
    /// * `/norestart` — never reboot the machine on its own. A disk
    ///   analyser that restarts somebody's computer to finish updating
    ///   itself would be indefensible. If files really are in use the install
    ///   fails and is reported, which is recoverable; a reboot is not.
    /// * `/l*v` — a verbose log beside the package. Nothing reads it, and that
    ///   is the point: when this fails on a machine nobody here owns, the log
    ///   is the only thing that can say why.
    pub fn for_package(package: &Path) -> Self {
        let log = package.with_extension("log");
        Self {
            program: OsString::from("msiexec"),
            args: vec![
                OsString::from("/i"),
                package.as_os_str().to_owned(),
                OsString::from("/qn"),
                OsString::from("/norestart"),
                OsString::from("/l*v"),
                log.as_os_str().to_owned(),
            ],
            log,
        }
    }

    /// The arguments as one string, for `ShellExecuteExW`, which takes the
    /// parameters as a single line rather than as a vector.
    ///
    /// **A path is quoted and a switch never is**, and that asymmetry is
    /// `msiexec`'s rather than a style. A package under
    /// `C:\Users\Someone Else\...` has a space in it and an unquoted path is
    /// read as two arguments — an installer that reports a missing file on
    /// exactly the machines whose owner has a space in their name. But
    /// `msiexec` parses its own command line instead of going through
    /// `CommandLineToArgvW`, and it does not recognise a **quoted** switch: it
    /// saw no `/i` and no `/qn`, put up its usage dialog, and then waited to be
    /// dismissed. Under elevation that dialog is on another desktop's window
    /// list and effectively invisible, so the install sat at nothing for ever
    /// while `WaitForSingleObject` waited on it. That was the symptom reported
    /// from a real machine: consent given, and then nothing at all.
    ///
    /// Leading `/` is the test because that is what `msiexec` itself treats as
    /// a switch. Everything else is a path and gets its quotes.
    pub fn parameters(&self) -> String {
        self.args
            .iter()
            .map(|a| {
                let text = a.to_string_lossy();
                if text.starts_with('/') {
                    text.into_owned()
                } else {
                    format!("\"{text}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// How far the install has got, for the window to draw.
///
/// A short list on purpose. Windows Installer reports nothing useful to a
/// caller running it silently — there is no progress channel out of `/qn` — so
/// what is honest here is *which step*, never a percentage. That is the rule
/// `Stage::progress` already follows for `HandingOver`: a bar that animates
/// over something it cannot see is the control this project refuses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// In place, and the user will have to start it themselves.
    ///
    /// A **success**, and keeping it out of [`Step::Failed`] is the point. The
    /// package went on exactly as asked; all that is missing is the relaunch,
    /// which for a first install is a nicety. Reporting that as a failed
    /// installation would send somebody looking for a problem that is not
    /// there — the same rule `relaunch` follows, where an update that could not
    /// restart leaves the old Heft running rather than claiming it failed.
    Installed,
    /// Waiting to be told to start.
    ///
    /// The first-run installer alone. An update was asked for and carries on;
    /// setup was double-clicked, and putting files on somebody's machine
    /// because they opened a window would be the wrong way round.
    Ready,
    /// Waiting for Heft to close. Fast, and the one step that could hang, so
    /// it is named rather than folded into the next.
    WaitingForHeft,
    /// The consent prompt is up, or about to be.
    AskingPermission,
    /// `msiexec` is running.
    Installing,
    /// Done, and the new Heft is being started.
    Starting,
    /// Done, and this window is about to close.
    Finished,
    /// It did not work. The window says so, names the log and offers to open
    /// the folder — rather than closing and leaving somebody with the version
    /// they had and no idea why.
    Failed(String),
}

impl Step {
    /// The line under the bar.
    pub fn label(&self) -> String {
        match self {
            Self::Ready => "Heft will be installed for everyone on this machine.".to_string(),
            Self::Installed => "Heft is installed. Start it from the Start menu.".to_string(),
            Self::WaitingForHeft => "Waiting for Heft to close...".to_string(),
            Self::AskingPermission => "Asking Windows for permission to install...".to_string(),
            Self::Installing => "Installing...".to_string(),
            Self::Starting => "Starting the new version...".to_string(),
            Self::Finished => "Done.".to_string(),
            Self::Failed(why) => why.clone(),
        }
    }

    /// Whether the window is still working, and therefore may not be dismissed.
    ///
    /// The same rule `Flow::holds_work` follows in the update dialog: a window
    /// that vanished mid-install would leave `msiexec` running with nothing on
    /// screen to say so.
    pub fn holds_work(&self) -> bool {
        !matches!(
            self,
            Self::Ready | Self::Installed | Self::Finished | Self::Failed(_)
        )
    }

    /// How far along the bar sits, where that can be said at all.
    ///
    /// `None` while `msiexec` runs, because it reports nothing and an animation
    /// invented here would be a lie about somebody's installation. The bar
    /// draws an empty track, exactly as `Stage::HandingOver`'s does.
    pub fn progress(&self) -> Option<f32> {
        match self {
            Self::Ready => Some(0.0),
            Self::Installed => Some(1.0),
            Self::WaitingForHeft => Some(0.1),
            Self::AskingPermission => Some(0.25),
            Self::Installing => None,
            Self::Starting => Some(0.95),
            Self::Finished => Some(1.0),
            Self::Failed(_) => None,
        }
    }
}

/// The folder name the package installs into, under Program Files.
///
/// Pinned against `packaging/windows/heft.wxs` by a test rather than merely
/// matching it today: the setup window starts Heft from this path when the
/// package has gone in, and a rename over there would otherwise turn a
/// successful install into a window that could not find what it had installed.
/// Same arrangement `taskbar`'s names keep against `packaging/`.
pub const INSTALL_FOLDER: &str = "Heft";

/// Where the package puts `heft.exe`, given what Windows says Program Files
/// is.
///
/// Injected rather than read from the environment here, for the reason
/// [`super::install::detect`] takes a `Probe`: it is the only way the answer is
/// testable on a machine that is not Windows.
///
/// **Only for the first install.** An update knows the path already — it is the
/// Heft that spawned the helper — and carries it in [`Job::target`]. This is
/// the case where there was no Heft to ask.
pub fn installed_path(program_files: Option<&str>) -> Option<PathBuf> {
    let root = program_files?;
    if root.is_empty() {
        return None;
    }
    Some(PathBuf::from(root).join(INSTALL_FOLDER).join("heft.exe"))
}

/// Put a copy of this executable somewhere the installer will not be replacing
/// it, and answer where it went.
///
/// The helper must not run from inside the installation being upgraded: a
/// running file is in use, and Windows Installer meeting one either schedules a
/// reboot or has Restart Manager kill it — with the update half done and the
/// window that was explaining it gone.
pub fn stage_helper(dir: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Heft could not find its own program file: {e}"))?;
    let name = if cfg!(windows) {
        "heft-updater.exe"
    } else {
        "heft-updater"
    };
    let to = dir.join(name);
    // Removed first rather than overwritten: a copy left by an update that was
    // abandoned may still be running, and on Windows the write would fail with
    // a sharing violation that reads like a permissions problem.
    let _ = std::fs::remove_file(&to);
    std::fs::copy(&exe, &to)
        .map_err(|e| format!("Heft could not stage the updater at {}: {e}", to.display()))?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    /// **A setup binary is double-clicked, so it has no command line.**
    ///
    /// This is the whole of the bug it pins: dispatch asked the arguments and
    /// nothing else, `--install` is never passed by anybody, and so
    /// `heft-setup.exe` started as the application and its installer could not
    /// be reached at all. Reported from a real machine.
    ///
    /// The other three rows are the reason this is a function of two readings
    /// rather than a second check bolted after the first: an argument has to
    /// win, or an update's helper would be diverted into setup by the payload
    /// the setup binary it was copied from is carrying.
    #[test]
    fn a_payload_makes_setup_of_a_launch_with_no_arguments() {
        // The reported bug: no argument, a package on the end.
        let launch = job(args(&["heft-setup.exe"]), true).expect("setup was not recognised");
        assert!(launch.setup, "a payload with no argument is not setup");
        assert_eq!(
            launch.package,
            PathBuf::new(),
            "setup carries its own package"
        );

        // Ordinary Heft: no argument, no payload.
        assert!(job(args(&["heft.exe"]), false).is_none());

        // An argument wins over a payload, both ways round.
        let helper = job(args(&["heft.exe", FLAG, "C:/tmp/heft.msi", "4242"]), true)
            .expect("the helper's own flag was ignored");
        assert!(!helper.setup, "a payload diverted an update into setup");
        assert_eq!(helper.package, PathBuf::from("C:/tmp/heft.msi"));
        assert_eq!(helper.parent, Some(4242));

        // And the flag still asks for setup deliberately, with no payload seen.
        assert!(
            job(args(&["heft.exe", SETUP_FLAG]), false)
                .expect("the flag stopped working")
                .setup
        );

        // Both routes build the identical job, which is why `Job::setup` exists.
        assert_eq!(
            job(args(&["heft-setup.exe"]), true).map(|j| j.setup),
            job(args(&["heft.exe", SETUP_FLAG]), false).map(|j| j.setup),
        );
    }

    /// An ordinary launch is not the installer, and neither is one carrying
    /// somebody else's argument.
    #[test]
    fn an_ordinary_command_line_is_not_an_install() {
        assert_eq!(parse(args(&["heft"])), None);
        assert_eq!(parse(args(&[])), None);
        assert_eq!(parse(args(&["heft", "C:\\Users"])), None);
        // The text mode's commands must fall through to the text mode, not be
        // swallowed here.
        assert_eq!(parse(args(&["heft", "scan", "--json"])), None);
    }

    /// **The first-run installer needs no arguments**, because the package is
    /// on the end of the file it is reading from. That is what lets it be
    /// double-clicked.
    #[test]
    fn setup_is_its_own_flag_and_carries_nothing() {
        let job = parse(args(&["heft-setup.exe", SETUP_FLAG])).expect("setup");
        assert!(job.setup);
        assert_eq!(job.parent, None);
        assert_eq!(job.target, None);

        // And it is not the update flag: an update names a package and waits
        // for the process that spawned it, and confusing the two would install
        // nothing or install it twice.
        let update = parse(args(&["heft", FLAG, "p.msi"])).expect("update");
        assert!(!update.setup);
    }

    #[test]
    fn the_flag_carries_a_package_a_parent_and_a_version() {
        let job = parse(args(&[
            "heft",
            FLAG,
            "C:\\Temp\\heft-0.0.8-x64.msi",
            "4321",
            "0.0.8",
        ]))
        .expect("an install");
        assert_eq!(job.package, PathBuf::from("C:\\Temp\\heft-0.0.8-x64.msi"));
        assert_eq!(job.parent, Some(4321));
        assert_eq!(job.version, "0.0.8");
    }

    /// **The flag with nothing usable after it is not an install.** A window
    /// that opened with no package to put in place could only report its own
    /// arguments, where starting Heft is something somebody can use.
    #[test]
    fn a_flag_with_no_package_is_refused() {
        assert_eq!(parse(args(&["heft", FLAG])), None);
        assert_eq!(parse(args(&["heft", FLAG, ""])), None);

        // The trailing two are optional: started by hand, with no parent to
        // wait for and no version to name.
        let job = parse(args(&["heft", FLAG, "p.msi"])).expect("an install");
        assert_eq!(job.parent, None);
        assert_eq!(job.version, "");
        assert_eq!(job.target, None);
        assert!(!job.setup);

        // And a parent that is not a number is "do not wait" rather than a
        // refusal: this is a command line, and refusing to run over one bad word
        // is worse than running.
        let job = parse(args(&["heft", FLAG, "p.msi", "later"])).expect("an install");
        assert_eq!(job.parent, None);
    }

    /// **Silent, never rebooting, and logged.** Each of these is load-bearing
    /// and each would be invisible if it regressed: `/qn` is the whole feature,
    /// `/norestart` is the difference between a failed update and somebody's
    /// machine restarting under them, and the log is the only evidence
    /// available when this fails on hardware nobody here owns.
    #[test]
    fn the_install_is_silent_and_never_reboots() {
        let cmd = Command::for_package(Path::new("C:\\Temp\\heft-0.0.8-x64.msi"));
        let flat: Vec<String> = cmd
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(flat.contains(&"/qn".to_string()), "{flat:?}");
        assert!(flat.contains(&"/norestart".to_string()), "{flat:?}");
        assert!(flat.contains(&"/i".to_string()), "{flat:?}");
        assert_eq!(cmd.log, PathBuf::from("C:\\Temp\\heft-0.0.8-x64.log"));
        // No interactive level may creep back in beside the silent one.
        for loud in ["/qb", "/qf", "/qr", "/passive"] {
            assert!(!flat.iter().any(|a| a == loud), "{loud} in {flat:?}");
        }
    }

    /// A path with a space in it is one argument, and a switch is bare.
    ///
    /// This test used to assert the opposite of its second half — it required
    /// `"/i"` quoted, and so held the defect in place rather than catching it.
    /// `msiexec` parses its own command line and does not recognise a quoted
    /// switch: it saw no `/i` and no `/qn`, raised its usage dialog where an
    /// elevated process's window could not be seen, and waited there. The
    /// install sat at nothing for ever after consent was given.
    #[test]
    fn a_path_is_quoted_and_a_switch_is_not() {
        let cmd = Command::for_package(Path::new("C:\\Users\\Some One\\heft.msi"));
        let line = cmd.parameters();

        // The reason quoting exists at all.
        assert!(
            line.contains("\"C:\\Users\\Some One\\heft.msi\""),
            "the package path lost its quotes: {line}"
        );
        assert!(
            line.contains("\"C:\\Users\\Some One\\heft.log\""),
            "the log path lost its quotes: {line}"
        );

        // And the half that was backwards. Every switch bare, none of them
        // wearing quotes msiexec would refuse to read.
        for switch in ["/i", "/qn", "/norestart", "/l*v"] {
            assert!(
                line.contains(&format!(" {switch} ")) || line.starts_with(&format!("{switch} ")),
                "{switch} is missing or quoted: {line}"
            );
            assert!(
                !line.contains(&format!("\"{switch}\"")),
                "{switch} is quoted, which msiexec will not read: {line}"
            );
        }
        assert!(line.starts_with("/i "), "{line}");
    }

    /// **A successful install is never reported as a failure**, however the
    /// relaunch went. `Installed` is the arm that says so.
    #[test]
    fn being_unable_to_start_heft_is_not_a_failed_install() {
        assert!(!Step::Installed.holds_work());
        assert_eq!(Step::Installed.progress(), Some(1.0));
        let said = Step::Installed.label().to_lowercase();
        for word in ["fail", "could not", "error", "sorry", "problem"] {
            assert!(!said.contains(word), "{said:?} reads as a failure");
        }
    }

    /// Where the first install starts Heft from, and it must be where the
    /// package actually put it.
    ///
    /// **Read as components rather than compared against a spelled-out path**,
    /// which is what this did first and is a test that only passes on Windows:
    /// a backslash is not a separator on Unix, so `PathBuf::from(r"C:\Program
    /// Files")` is *one* component there and joining two more onto it produced
    /// `C:\Program Files/Heft/heft.exe` against an expected literal of one
    /// piece. It failed on all three Unix runners and passed on both Windows
    /// ones, which is exactly the shape of failure the release script's CI wait
    /// exists to catch — and did.
    ///
    /// The root has no drive letter for the same reason: nothing here is about
    /// how a platform spells a path, only that the folder and the binary are
    /// appended to it in that order.
    #[test]
    fn the_installed_path_is_program_files_and_the_packages_own_folder() {
        let path = installed_path(Some("root")).expect("a path");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert_eq!(parts, ["root", INSTALL_FOLDER, "heft.exe"]);

        // Nothing to build a path from is "do not guess", which the window
        // reads as `Installed` rather than as a failure.
        assert_eq!(installed_path(None), None);
        assert_eq!(installed_path(Some("")), None);
    }

    /// **The folder is the package's**, and this is what stops the two drifting:
    /// renaming it in the `.wxs` without changing it here would leave a
    /// successful install unable to find what it had just installed.
    /// Heft's own MSI identity.
    ///
    /// Pinned here so that a `.wxs` copied from a sibling application fails the
    /// build instead of shipping. Muster 0.0.1 went out carrying Umber's
    /// UpgradeCode, which made Windows treat the two as one product. Heft's
    /// were generated for it when the file was ported from Muster's. See the
    /// comment at the head of `packaging/windows/heft.wxs`.
    const UPGRADE_CODE: &str = "597d1c7e-3e94-41e3-ad30-776a363cd080";

    /// Every GUID the family's other MSIs carry (Muster, Umber, Windup,
    /// Brokey: upgrade codes and component GUIDs), as of the port. Heft's
    /// file was copied from Muster's, and this is what fails if a GUID came
    /// with it.
    const SIBLINGS: &[&str] = &[
        "0a3bb131-b281-44ef-b637-3af01bbe1c20",
        "0c8cd24b-8e1e-4bad-8e63-e4a1ebe962f9",
        "0db79ce2-ba97-4874-ba64-e0499572c253",
        "1e5a97c4-8362-4b0d-95fa-6c48d7e230b1",
        "2c076a68-85ff-4968-bc0b-5e05de1f1dae",
        "2d84b60f-5e73-4a96-9f21-84b5c6e0a7d2",
        "337ffc2c-254e-4f27-b07b-607345a9a4b6",
        "38d24698-fd9a-4f4c-9782-b6d197a80fa7",
        "434874ef-1ff3-4579-8978-cbd8ac1e9654",
        "4a1d8e05-63b2-4c7f-9e18-27fb0a5d3c64",
        "67424759-2b1e-4242-951c-2573a3aa2da6",
        "67a3c020-8956-4ddb-ae5f-f227bfd93768",
        "6b24f8d0-937c-4e51-82a6-04ec1b95d7f3",
        "6f3a1c2e-9b47-4d18-8a52-1c0e77d4b301",
        "7c05d2a8-4318-49be-b6f0-9d21e83c5a4f",
        "7e5ebbd5-584b-4ba2-8354-344b842d3907",
        "8f0b6d27-4e15-49c3-a7d0-3b26e5c81a94",
        "95c43486-778e-4975-9cb5-112e6ba93260",
        "9c2f5b31-4de8-47a6-b0d1-5e3a8f7c26b4",
        "9e467a98-24b3-439a-aea0-67df3e57da64",
        "aaa698ce-4d5d-41c6-8f1e-5098c7fffb1b",
        "abfd9685-beb8-4418-829b-90f3421873c6",
        "b1e7f394-0c62-4f85-9d47-3ea8250b16c9",
        "c73e4b19-05af-4d62-8b3e-91d7a4b60e28",
        "dbec9cf7-f410-4fd2-819d-0cb0511b54a7",
        "e357fccd-a995-4576-b01f-234630154e96",
        "e94a6b71-8d3f-4c02-a15e-62b7f0d4938a",
        "f3fbcac2-a3ac-4cf2-99fc-1dfa05429952",
        "f9aca005-ce23-4a9f-bd1b-ce170bf28696",
    ];

    fn wxs() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packaging/windows/heft.wxs"),
        )
        .expect("the packaging file")
    }

    #[test]
    fn the_package_carries_hefts_own_upgrade_code() {
        let wxs = wxs();
        assert!(
            wxs.contains(&format!("UpgradeCode=\"{UPGRADE_CODE}\"")),
            "heft.wxs does not carry Heft's UpgradeCode {UPGRADE_CODE}. If this \
             file was copied from another application, its identity came with \
             it, and Windows will treat the two as the same product."
        );
    }

    #[test]
    fn no_guid_in_the_package_belongs_to_a_sibling() {
        let wxs = wxs().to_lowercase();
        for guid in SIBLINGS {
            assert!(
                !wxs.contains(guid),
                "heft.wxs carries {guid}, which another application's MSI uses"
            );
        }
    }

    #[test]
    fn no_guid_in_the_package_is_used_twice() {
        // Component GUIDs are what MSI reference-counts installed files by. Two
        // components sharing one means uninstalling either can take the other's
        // files, which is a defect nothing reports at build time.
        let wxs = wxs();
        let mut guids: Vec<&str> = wxs
            .split(['"', '\''])
            .filter(|token| {
                token.len() == 36
                    && token.chars().enumerate().all(|(i, c)| match i {
                        8 | 13 | 18 | 23 => c == '-',
                        _ => c.is_ascii_hexdigit(),
                    })
            })
            .collect();
        let before = guids.len();
        assert!(before >= 3, "expected the package to carry GUIDs");
        guids.sort_unstable();
        guids.dedup();
        assert_eq!(
            guids.len(),
            before,
            "heft.wxs uses the same GUID for two different things"
        );
    }

    #[test]
    fn the_install_folder_is_the_one_the_package_uses() {
        let wxs = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packaging/windows/heft.wxs"),
        )
        .expect("the packaging file");
        let wanted = format!("<Directory Id=\"INSTALLFOLDER\" Name=\"{INSTALL_FOLDER}\" />");
        assert!(
            wxs.contains(&wanted),
            "heft.wxs does not install into {INSTALL_FOLDER}; expected {wanted}"
        );
    }

    /// The window may not be dismissed while `msiexec` is running, and must be
    /// dismissible the moment it is not.
    #[test]
    fn a_step_that_is_working_holds_the_window() {
        for step in [
            Step::WaitingForHeft,
            Step::AskingPermission,
            Step::Installing,
            Step::Starting,
        ] {
            assert!(step.holds_work(), "{step:?}");
        }
        assert!(!Step::Finished.holds_work());
        assert!(!Step::Failed("no".into()).holds_work());
        // Setup before it has been told to start is not working either: the
        // window has a Cancel that must not be refused.
        assert!(!Step::Ready.holds_work());
    }

    /// **The bar is empty while Windows is installing**, because nothing
    /// reports progress out of a silent `msiexec` and a bar that moved anyway
    /// would be inventing it.
    #[test]
    fn nothing_claims_progress_it_cannot_see() {
        assert_eq!(Step::Installing.progress(), None);
        assert_eq!(Step::Failed("x".into()).progress(), None);
        assert_eq!(Step::Finished.progress(), Some(1.0));
    }

    /// No step may claim the download or the package was checked in a way it
    /// was not — `update`'s standing rule, applied to this window's wording
    /// too. Heft does not sign its releases.
    #[test]
    fn no_step_calls_anything_verified() {
        for step in [
            Step::Ready,
            Step::Installed,
            Step::WaitingForHeft,
            Step::AskingPermission,
            Step::Installing,
            Step::Starting,
            Step::Finished,
        ] {
            let said = step.label().to_lowercase();
            for word in ["verif", "authentic", "secure", "signed", "signature"] {
                assert!(!said.contains(word), "{said:?} contains {word}");
            }
        }
    }
}
