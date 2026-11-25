//! Every command, option and argument the CLI accepts, one function per
//! command so each stays readable on its own.

use clap::{Arg, ArgAction, Command};

use crate::edit;
use crate::text::t;

fn paths() -> Arg {
    Arg::new("paths")
        .help(t("argPaths", &[]))
        .num_args(1..)
        .required(true)
        .value_parser(clap::value_parser!(std::path::PathBuf))
        .value_hint(clap::ValueHint::AnyPath)
}
fn rule_sources(c: Command) -> Command {
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
}
fn list_options(c: Command) -> Command {
    list_choose(list_edit(list_filter(c)))
}

/// What of the list is shown, and in what order.
fn list_filter(c: Command) -> Command {
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
}

/// The names and details typed in, and where files go.
fn list_edit(c: Command) -> Command {
    c.arg(
        Arg::new("name")
            .long("name")
            .help(t("argName", &[]))
            .action(ArgAction::Append)
            .value_name("PATH=NAME"),
    )
    .arg(
        Arg::new("edit")
            .long("edit")
            .help(t(
                "argEdit",
                &[
                    ("music", &edit::names("music")),
                    ("book", &edit::names("book")),
                    ("comic", &edit::names("comic")),
                    ("photo", &edit::names("photo")),
                ],
            ))
            .action(ArgAction::Append)
            .value_name("PATH:KEY=VALUE,…"),
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
}

/// What is looked up, how doubts are settled, and the lists written.
fn list_choose(c: Command) -> Command {
    c.arg(
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
    .arg(
        Arg::new("missing-csv")
            .long("missing-csv")
            .help(t("argMissingCsv", &[]))
            .value_name("FILE")
            .value_parser(clap::value_parser!(std::path::PathBuf))
            .value_hint(clap::ValueHint::FilePath),
    )
}
fn id() -> Arg {
    Arg::new("id").help(t("argId", &[]))
}
fn set_name() -> Arg {
    Arg::new("name").help(t("argSetName", &[])).required(true)
}
fn option_name() -> Arg {
    Arg::new("option").help(t("argOption", &[])).required(true)
}
fn key_name() -> Arg {
    Arg::new("key").help(t("argKey", &[])).required(true)
}

fn preview() -> Command {
    list_options(rule_sources(
        Command::new("preview")
            .about(t("cmdPreview", &[]))
            .arg(paths()),
    ))
}

fn apply() -> Command {
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
    )
}

fn undo() -> Command {
    Command::new("undo").about(t("cmdUndo", &[])).arg(id())
}

fn history() -> Command {
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
        .subcommand(Command::new("clear").about(t("histClear", &[])))
}

fn sets() -> Command {
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
        )
}

fn rules() -> Command {
    Command::new("rules")
        .about(t("cmdRules", &[]))
        .arg(Arg::new("type").help(t("argRuleType", &[])))
}

fn options() -> Command {
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
        )
}

fn keys() -> Command {
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
        )
        .subcommand(
            Command::new("test")
                .about(t("keysTest", &[]))
                .arg(key_name()),
        )
}

fn servers() -> Command {
    let id = || Arg::new("id").help(t("argServerId", &[])).required(true);
    let server_path = || {
        Arg::new("server")
            .help(t("argServerPath", &[]))
            .required(true)
    };
    Command::new("servers")
        .about(t("cmdServers", &[]))
        .subcommand(Command::new("list").about(t("srvList", &[])))
        .subcommand(
            Command::new("add")
                .about(t("srvAdd", &[]))
                .arg(
                    Arg::new("kind")
                        .help(t("argServerKind", &[]))
                        .required(true)
                        .value_parser(
                            renome_services::mediaserver::KINDS
                                .iter()
                                .map(|k| k.0)
                                .collect::<Vec<_>>(),
                        ),
                )
                .arg(Arg::new("url").help(t("argServerUrl", &[])).required(true))
                .arg(
                    Arg::new("stdin")
                        .long("stdin")
                        .help(t("argStdin", &[]))
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(Command::new("remove").about(t("srvRemove", &[])).arg(id()))
        .subcommand(
            Command::new("libraries")
                .about(t("srvLibraries", &[]))
                .arg(id()),
        )
        .subcommand(
            Command::new("pair")
                .about(t("srvPair", &[]))
                .arg(id())
                .arg(server_path())
                .arg(
                    Arg::new("local")
                        .help(t("argLocalPath", &[]))
                        .required(true)
                        .value_hint(clap::ValueHint::DirPath),
                ),
        )
        .subcommand(
            Command::new("on")
                .about(t("srvOn", &[]))
                .arg(id())
                .arg(server_path()),
        )
        .subcommand(
            Command::new("off")
                .about(t("srvOff", &[]))
                .arg(id())
                .arg(server_path()),
        )
        .subcommand(Command::new("test").about(t("srvTest", &[])).arg(id()))
}

fn duplicates() -> Command {
    Command::new("duplicates")
        .about(t("cmdDuplicates", &[]))
        .arg(paths().required(false).required_unless_present("trash"))
        .arg(
            Arg::new("identical")
                .long("identical")
                .help(t("argIdentical", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("repeated")
                .long("repeated")
                .help(t("argRepeated", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("similar")
                .long("similar")
                .help(t("argSimilar", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("trash")
                .long("trash")
                .help(t("argTrash", &[]))
                .num_args(1..)
                .value_name("PATH")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .help(t("argYes", &[]))
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
}

fn update() -> Command {
    Command::new("update")
        .about(t("cmdUpdate", &[]))
        .subcommand(Command::new("check").about(t("updCheck", &[])))
        .subcommand(Command::new("install").about(t("updInstall", &[])))
}

fn manual() -> Command {
    Command::new("manual")
        .about(t("cmdManual", &[]))
        .arg(Arg::new("topic").help(t("argTopic", &[])))
        .arg(
            Arg::new("all")
                .long("all")
                .help(t("argManualAll", &[]))
                .action(ArgAction::SetTrue),
        )
}

fn cache() -> Command {
    Command::new("cache")
        .about(t("cmdCache", &[]))
        .subcommand(Command::new("size").about(t("cacheSize", &[])))
        .subcommand(Command::new("clear").about(t("cacheClear", &[])))
}

fn manpages() -> Command {
    Command::new("manpages").hide(true).arg(
        Arg::new("dir")
            .required(true)
            .value_parser(clap::value_parser!(std::path::PathBuf)),
    )
}

fn completions() -> Command {
    Command::new("completions")
        .about(t("cmdCompletions", &[]))
        .arg(
            Arg::new("shell")
                .help(t("argShell", &[]))
                .required(true)
                .value_parser(clap::value_parser!(clap_complete::Shell)),
        )
}

pub fn commands() -> Command {
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
            Arg::new("no-server")
                .long("no-server")
                .global(true)
                .help(t("argNoServer", &[]))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .global(true)
                .help(t("argQuiet", &[]))
                .action(ArgAction::SetTrue),
        )
        .subcommand(preview())
        .subcommand(apply())
        .subcommand(undo())
        .subcommand(history())
        .subcommand(sets())
        .subcommand(rules())
        .subcommand(options())
        .subcommand(keys())
        .subcommand(servers())
        .subcommand(duplicates())
        .subcommand(update())
        .subcommand(manual())
        .subcommand(cache())
        .subcommand(manpages())
        .subcommand(completions())
}
