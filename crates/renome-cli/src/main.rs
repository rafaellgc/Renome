//! `renome-cli`: Renome from the terminal. The same rules, lookups,
//! history and options as the window, with the same code (see SPEC §8).

mod duplicates;
mod edit;
mod lookup;
mod manage;
mod manual;
mod missing;
mod out;
mod rulespec;
mod run;
mod servers;
mod spec;
mod stop;
mod text;
mod update;

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::process::ExitCode;

use crate::text::t;

/// How a run ends (SPEC §8.2).
pub enum Exit {
    Done,
    Failed,
    Doubts,
    Usage,
    /// Ctrl+C: what was done stays done (and in the History).
    Stopped,
}

impl From<Exit> for ExitCode {
    fn from(e: Exit) -> Self {
        ExitCode::from(match e {
            Exit::Done => 0,
            Exit::Failed => 1,
            Exit::Doubts => 2,
            Exit::Usage => 3,
            Exit::Stopped => 130,
        })
    }
}

fn main() -> ExitCode {
    // Output piped into `head` and the like just stops when they have
    // enough, as other terminal programs do.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let args: Vec<String> = std::env::args().collect();
    let settings = renome_services::read_settings_file();
    let asked = args
        .iter()
        .position(|a| a == "--lang")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .or_else(|| args.iter().find_map(|a| a.strip_prefix("--lang=")));
    text::set_lang(text::pick_lang(asked, settings.as_ref()));

    let matches = match cli().try_get_matches_from(&args) {
        Ok(m) => m,
        Err(e) => {
            let usage = e.use_stderr();
            let _ = e.print();
            return if usage {
                Exit::Usage.into()
            } else {
                Exit::Done.into()
            };
        }
    };
    out::setup(&matches);
    stop::install();
    let result = match matches.subcommand() {
        Some(("preview", m)) => run::preview_or_apply(m, settings.as_ref(), false),
        Some(("apply", m)) => run::preview_or_apply(m, settings.as_ref(), true),
        Some(("undo", m)) => manage::undo(m),
        Some(("history", m)) => manage::history(m),
        Some(("sets", m)) => manage::sets(m, settings),
        Some(("rules", m)) => manage::rules(m),
        Some(("options", m)) => manage::options(m, settings),
        Some(("keys", m)) => manage::keys(m),
        Some(("cache", m)) => manage::cache(m),
        Some(("manual", m)) => manual::command(m),
        Some(("update", m)) => update::command(m),
        Some(("duplicates", m)) => duplicates::command(m, settings.as_ref()),
        Some(("servers", m)) => tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())
            .and_then(|rt| servers::command(m, &rt)),
        Some(("help", m)) => {
            let mut c = cli();
            let words: Vec<&String> = m
                .get_many::<String>("command")
                .map(|v| v.collect())
                .unwrap_or_default();
            for w in words {
                match c.find_subcommand(w) {
                    Some(sub) => {
                        c = sub
                            .clone()
                            .bin_name(format!("{} {w}", c.get_bin_name().unwrap_or("renome-cli")))
                    }
                    None => return Exit::Usage.into(),
                }
            }
            let _ = c.print_help();
            Ok(Exit::Done)
        }
        Some(("manpages", m)) => man_pages(m.get_one::<std::path::PathBuf>("dir").unwrap()),
        Some(("completions", m)) => {
            let shell = *m.get_one::<clap_complete::Shell>("shell").unwrap();
            clap_complete::generate(shell, &mut cli(), "renome-cli", &mut std::io::stdout());
            Ok(Exit::Done)
        }
        _ => Ok(Exit::Done),
    };
    match result {
        Ok(exit) => exit.into(),
        Err(e) => {
            out::error(&e);
            Exit::Usage.into()
        }
    }
}

/// The commands and their options, with help in the language in use.
pub fn cli() -> Command {
    localized(spec::commands())
}

/// clap's own words (Usage, Options, Commands, help, version) in the
/// language chosen, on every command; `help` is ours, to say it too.
fn localized(c: Command) -> Command {
    let template = format!(
        "{{about-with-newline}}\n{}: {{usage}}\n\n{{all-args}}{{after-help}}",
        t("helpUsage", &[])
    );
    let top = c.get_name() == "renome-cli";
    let mut c = c
        .help_template(template)
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .subcommand_help_heading(t("helpCommands", &[]))
        .subcommand_value_name(t("helpCommand", &[]).to_uppercase())
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .help(t("helpHelp", &[]))
                .action(ArgAction::Help),
        )
        .mut_args(|a| {
            if a.get_help_heading().is_some() {
                a
            } else if a.is_positional() {
                a.help_heading(t("helpArgs", &[]))
            } else {
                a.help_heading(t("helpOptions", &[]))
            }
        })
        .mut_subcommands(localized);
    if top {
        c = c
            .disable_version_flag(true)
            .arg(
                Arg::new("version")
                    .short('V')
                    .long("version")
                    .help(t("helpVersion", &[]))
                    .action(ArgAction::Version)
                    .help_heading(t("helpOptions", &[])),
            )
            .subcommand(
                Command::new("help")
                    .about(t("helpHelpCmd", &[]))
                    .arg(
                        Arg::new("command")
                            .num_args(0..)
                            .help(t("helpCommand", &[]))
                            .help_heading(t("helpArgs", &[])),
                    )
                    .arg(
                        Arg::new("help")
                            .short('h')
                            .long("help")
                            .help(t("helpHelp", &[]))
                            .action(ArgAction::Help)
                            .help_heading(t("helpOptions", &[])),
                    )
                    .help_template(format!(
                        "{{about-with-newline}}\n{}: {{usage}}\n\n{{all-args}}",
                        t("helpUsage", &[])
                    )),
            );
    }
    c
}

/// `renome-cli.1` and one page per command (`renome-cli-sets-save.1`), for
/// the packages; in the language chosen (`--lang en` for them).
fn man_pages(dir: &std::path::Path) -> Result<Exit, String> {
    fn write(cmd: &Command, words: &[&str], dir: &std::path::Path) -> Result<(), String> {
        let name = words.join("-");
        let cmd = cmd.clone().name(name.clone()).bin_name(words.join(" "));
        let mut page = Vec::new();
        clap_mangen::Man::new(cmd.clone())
            .render(&mut page)
            .map_err(|e| e.to_string())?;
        std::fs::write(dir.join(format!("{name}.1")), page).map_err(|e| e.to_string())?;
        for sub in cmd
            .get_subcommands()
            .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        {
            let mut next = words.to_vec();
            next.push(sub.get_name());
            write(sub, &next, dir)?;
        }
        Ok(())
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut root = cli();
    root.build();
    write(&root, &["renome-cli"], dir)?;
    Ok(Exit::Done)
}

/// The rule sources given, in the order they were typed.
pub fn sources(m: &ArgMatches) -> Vec<rulespec::Source> {
    let mut found: Vec<(usize, rulespec::Source)> = Vec::new();
    for id in ["rule", "set", "rules"] {
        if let (Some(values), Some(indices)) = (m.get_many::<String>(id), m.indices_of(id)) {
            for (v, i) in values.zip(indices) {
                found.push((
                    i,
                    match id {
                        "rule" => rulespec::Source::Spec(v.clone()),
                        "set" => rulespec::Source::Set(v.clone()),
                        _ => rulespec::Source::File(v.clone()),
                    },
                ));
            }
        }
    }
    if m.get_flag("saved") {
        found.push((m.index_of("saved").unwrap_or(0), rulespec::Source::Saved));
    }
    found.sort_by_key(|(i, _)| *i);
    found.into_iter().map(|(_, s)| s).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_commands_are_well_defined() {
        let _turn = crate::text::TEST_LANG
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for lang in ["pt", "en", "es"] {
            crate::text::set_lang(lang);
            super::cli().debug_assert();
        }
    }
}
