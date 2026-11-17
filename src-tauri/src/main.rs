// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `Renome.AppImage cli …` (or `renome cli …`): the command line beside
    // this program takes over. Not on Windows, where this program has no
    // console: renome-cli.exe is run there.
    #[cfg(unix)]
    if std::env::args_os().nth(1).is_some_and(|a| a == "cli") {
        use std::os::unix::process::CommandExt;
        let beside = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.join("renome-cli")));
        if let Some(cli) = beside.filter(|p| p.exists()) {
            let err = std::process::Command::new(cli)
                .args(std::env::args_os().skip(2))
                .exec();
            eprintln!("renome-cli: {err}");
        } else {
            eprintln!("renome-cli is not part of this build");
        }
        std::process::exit(3);
    }
    renome_lib::run()
}
