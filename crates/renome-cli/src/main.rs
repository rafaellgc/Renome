//! `renome-cli`: Renome from the terminal. The same rules, lookups,
//! history and options as the window, with the same code (see SPEC §8).

mod lookup;
mod manage;
mod missing;
mod out;
mod rulespec;
mod run;
mod text;

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::process::ExitCode;

use crate::text::t;

/// How a run ends (SPEC §8.2).
pub enum Exit {
    Done,
    Failed,
    Doubts,
    Usage,
}

impl From<Exit> for ExitCode {
    fn from(e: Exit) -> Self {
        ExitCode::from(match e {
            Exit::Done => 0,
            Exit::Failed => 1,
            Exit::Doubts => 2,
            Exit::Usage => 3,
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
    let paths = || {
        Arg::new("paths")
            .help(t("argPaths", &[]))
            .num_args(1..)
            .required(true)
            .value_parser(clap::value_parser!(std::path::PathBuf))
            .value_hint(clap::ValueHint::AnyPath)
    };
    let rule_sources = |c: Command| {
        c.arg(
            Arg::new("rule")
                .short('r')
                .long("rule")
                .help(t("argRule", &[]))
                .action(ArgAction::Append)
                .value_name("TYPE:OPTION=VALUE"),
        )
        .arg(
            Arg::new("set")
                .long("set")
                .help(t("argSet", &[]))
                .action(ArgAction::Append)
                .value_name("NAME"),
        )
        .arg(
            Arg::new("rules")
                .long("rules")
                .help(t("argRules", &[]))
                .action(ArgAction::Append)
                .value_name("FILE")
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            Arg::new("saved")
                .long("saved")
                .help(t("argSaved", &[]))
                .action(ArgAction::SetTrue),
        )
    };
    let list_options = |c: Command| {
        c.arg(
            Arg::new("filter")
                .long("filter")
                .help(t("argFilter", &[]))
                .value_name("EXPR"),
        )
        .arg(
            Arg::new("sort")
                .long("sort")
                .help(t("argSort", &[]))
                .value_parser(["name", "new", "ext", "dir", "size", "modified"]),
        )
        .arg(
            Arg::new("desc")
                .long("desc")
                .help(t("argDesc", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("hidden")
                .long("hidden")
                .help(t("argHidden", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-recursive")
                .long("no-recursive")
                .help(t("argNoRecursive", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("folders")
                .long("folders")
                .help(t("argFolders", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("name")
                .long("name")
                .help(t("argName", &[]))
                .action(ArgAction::Append)
                .value_name("PATH=NAME"),
        )
        .arg(
            Arg::new("to")
                .long("to")
                .help(t("argTo", &[]))
                .value_name("DIR")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .value_hint(clap::ValueHint::DirPath),
        )
        .arg(
            Arg::new("copy")
                .long("copy")
                .help(t("argCopy", &[]))
                .requires("to")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .help(t("argAll", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("missing")
                .long("missing")
                .help(t("argMissing", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("choose")
                .long("choose")
                .help(t("argChoose", &[]))
                .value_parser(["ask", "auto", "skip"]),
        )
        .arg(
            Arg::new("pick")
                .long("pick")
                .help(t("argPick", &[]))
                .action(ArgAction::Append)
                .value_name("PATH=ID"),
        )
        .arg(
            Arg::new("csv")
                .long("csv")
                .help(t("argCsv", &[]))
                .value_name("FILE")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .value_hint(clap::ValueHint::FilePath),
        )
    };
    let id = || Arg::new("id").help(t("argId", &[]));
    let set_name = || Arg::new("name").help(t("argSetName", &[])).required(true);
    let option_name = || Arg::new("option").help(t("argOption", &[])).required(true);
    let key_name = || Arg::new("key").help(t("argKey", &[])).required(true);

    Command::new("renome-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about(t("about", &[]))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("lang")
                .long("lang")
                .global(true)
                .help(t("argLang", &[]))
                .value_parser(["pt", "en", "es"]),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .help(t("argJson", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("color")
                .long("color")
                .global(true)
                .help(t("argColor", &[]))
                .value_parser(["auto", "always", "never"])
                .default_value("auto"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .global(true)
                .help(t("argQuiet", &[]))
                .action(ArgAction::SetTrue),
        )
        .subcommand(list_options(rule_sources(
            Command::new("preview")
                .about(t("cmdPreview", &[]))
                .arg(paths()),
        )))
        .subcommand(
            list_options(rule_sources(
                Command::new("apply").about(t("cmdApply", &[])).arg(paths()),
            ))
            .arg(
                Arg::new("yes")
                    .short('y')
                    .long("yes")
                    .help(t("argYes", &[]))
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("strict")
                    .long("strict")
                    .help(t("argStrict", &[]))
                    .action(ArgAction::SetTrue),
            ),
        )
        .subcommand(Command::new("undo").about(t("cmdUndo", &[])).arg(id()))
        .subcommand(
            Command::new("history")
                .about(t("cmdHistory", &[]))
                .subcommand(Command::new("list").about(t("histList", &[])))
                .subcommand(
                    Command::new("show")
                        .about(t("histShow", &[]))
                        .arg(id().required(true)),
                )
                .subcommand(
                    Command::new("delete")
                        .about(t("histDelete", &[]))
                        .arg(id().required(true)),
                )
                .subcommand(Command::new("clear").about(t("histClear", &[]))),
        )
        .subcommand(
            Command::new("sets")
                .about(t("cmdSets", &[]))
                .subcommand(Command::new("list").about(t("setsList", &[])))
                .subcommand(
                    Command::new("show")
                        .about(t("setsShow", &[]))
                        .arg(set_name()),
                )
                .subcommand(
                    rule_sources(
                        Command::new("save")
                            .about(t("setsSave", &[]))
                            .arg(set_name()),
                    )
                    .arg(
                        Arg::new("replace")
                            .long("replace")
                            .help(t("argReplace", &[]))
                            .action(ArgAction::SetTrue),
                    ),
                )
                .subcommand(
                    Command::new("export")
                        .about(t("setsExport", &[]))
                        .arg(set_name())
                        .arg(Arg::new("file").help(t("argFile", &[])).default_value("-")),
                )
                .subcommand(
                    Command::new("import")
                        .about(t("setsImport", &[]))
                        .arg(
                            Arg::new("file")
                                .help(t("argFile", &[]))
                                .required(true)
                                .value_hint(clap::ValueHint::FilePath),
                        )
                        .arg(Arg::new("name").long("name").help(t("argSetName", &[])))
                        .arg(
                            Arg::new("replace")
                                .long("replace")
                                .help(t("argReplace", &[]))
                                .action(ArgAction::SetTrue),
                        ),
                )
                .subcommand(
                    Command::new("delete")
                        .about(t("setsDelete", &[]))
                        .arg(set_name()),
                )
                .subcommand(
                    Command::new("rename")
                        .about(t("setsRename", &[]))
                        .arg(set_name())
                        .arg(Arg::new("new").help(t("argNewName", &[])).required(true)),
                ),
        )
        .subcommand(
            Command::new("rules")
                .about(t("cmdRules", &[]))
                .arg(Arg::new("type").help(t("argRuleType", &[]))),
        )
        .subcommand(
            Command::new("options")
                .about(t("cmdOptions", &[]))
                .subcommand(Command::new("list").about(t("optList", &[])))
                .subcommand(
                    Command::new("get")
                        .about(t("optGet", &[]))
                        .arg(option_name()),
                )
                .subcommand(
                    Command::new("set")
                        .about(t("optSet", &[]))
                        .arg(option_name())
                        .arg(Arg::new("value").help(t("argValue", &[])).required(true)),
                )
                .subcommand(
                    Command::new("reset")
                        .about(t("optReset", &[]))
                        .arg(option_name()),
                ),
        )
        .subcommand(
            Command::new("keys")
                .about(t("cmdKeys", &[]))
                .subcommand(Command::new("list").about(t("keysList", &[])))
                .subcommand(
                    Command::new("set")
                        .about(t("keysSet", &[]))
                        .arg(key_name())
                        .arg(
                            Arg::new("stdin")
                                .long("stdin")
                                .help(t("argStdin", &[]))
                                .action(ArgAction::SetTrue),
                        ),
                )
                .subcommand(
                    Command::new("delete")
                        .about(t("keysDelete", &[]))
                        .arg(key_name()),
                ),
        )
        .subcommand(
            Command::new("cache")
                .about(t("cmdCache", &[]))
                .subcommand(Command::new("size").about(t("cacheSize", &[])))
                .subcommand(Command::new("clear").about(t("cacheClear", &[]))),
        )
        .subcommand(
            Command::new("completions")
                .about(t("cmdCompletions", &[]))
                .arg(
                    Arg::new("shell")
                        .help(t("argShell", &[]))
                        .required(true)
                        .value_parser(clap::value_parser!(clap_complete::Shell)),
                ),
        )
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
