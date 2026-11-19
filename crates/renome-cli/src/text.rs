//! What the command line says, in Portuguese, English or Spanish: its own
//! messages ([`CLI`]) and the page's texts (`src/lib/i18n.ts`, read at
//! build time), with counts in brackets made singular or plural as the
//! page does (`{n} arquivo(s)`, `[tem|têm]`).

use std::sync::atomic::{AtomicU8, Ordering};

mod page {
    include!(concat!(env!("OUT_DIR"), "/texts.rs"));
}

/// 0 Portuguese, 1 English, 2 Spanish.
static LANG: AtomicU8 = AtomicU8::new(1);

/// The language for the rest of the run: `pt`, `en` or `es`.
pub fn set_lang(lang: &str) {
    LANG.store(
        match lang {
            "pt" => 0,
            "es" => 2,
            _ => 1,
        },
        Ordering::Relaxed,
    );
}

/// Tests that change the language take turns.
#[cfg(test)]
pub static TEST_LANG: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn lang() -> &'static str {
    ["pt", "en", "es"][LANG.load(Ordering::Relaxed) as usize]
}

/// The language asked for, else the options', else the system's.
pub fn pick_lang(asked: Option<&str>, settings: Option<&serde_json::Value>) -> &'static str {
    let from = |s: &str| -> Option<&'static str> {
        let s = s.to_ascii_lowercase();
        if s.starts_with("pt") {
            Some("pt")
        } else if s.starts_with("es") {
            Some("es")
        } else if s.starts_with("en") {
            Some("en")
        } else {
            None
        }
    };
    if let Some(l) = asked.and_then(from) {
        return l;
    }
    if let Some(l) = settings
        .and_then(|s| s.get("lang")?.as_str())
        .and_then(from)
    {
        return l;
    }
    ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|v| from(&v))
        .unwrap_or("en")
}

/// The command line's own messages: key, then Portuguese, English,
/// Spanish.
const CLI: &[(&str, [&str; 3])] = &[
    (
        "about",
        [
            "Renomeia arquivos em lote com as regras do Renome, pelo terminal.",
            "Batch-renames files with Renome's rules, from the terminal.",
            "Renombra archivos en lote con las reglas de Renome, desde el terminal.",
        ],
    ),
    (
        "cmdPreview",
        [
            "Mostra os nomes novos, sem mudar nada",
            "Shows the new names, changing nothing",
            "Muestra los nombres nuevos, sin cambiar nada",
        ],
    ),
    (
        "cmdApply",
        [
            "Renomeia (ou move, ou copia) os arquivos",
            "Renames (or moves, or copies) the files",
            "Renombra (o mueve, o copia) los archivos",
        ],
    ),
    (
        "cmdUndo",
        [
            "Desfaz uma renomeação do Histórico (a última, sem ID)",
            "Undoes a rename from the History (the last one, without an ID)",
            "Deshace un renombrado del Historial (el último, sin ID)",
        ],
    ),
    (
        "cmdHistory",
        [
            "Lista, mostra ou apaga entradas do Histórico",
            "Lists, shows or deletes History entries",
            "Lista, muestra o borra entradas del Historial",
        ],
    ),
    (
        "cmdSets",
        [
            "Conjuntos de regras salvos",
            "Saved rule sets",
            "Conjuntos de reglas guardados",
        ],
    ),
    (
        "cmdRules",
        [
            "Lista as regras, ou as opções de uma regra",
            "Lists the rules, or a rule's options",
            "Lista las reglas, o las opciones de una regla",
        ],
    ),
    (
        "cmdOptions",
        [
            "Lê e muda as Opções",
            "Reads and changes the Options",
            "Lee y cambia las Opciones",
        ],
    ),
    (
        "cmdKeys",
        [
            "Chaves e contas dos serviços (no chaveiro do sistema)",
            "Service keys and accounts (in the system's password store)",
            "Claves y cuentas de los servicios (en el almacén de contraseñas del sistema)",
        ],
    ),
    (
        "cmdCache",
        [
            "Respostas dos catálogos guardadas",
            "Kept catalogue answers",
            "Respuestas de los catálogos guardadas",
        ],
    ),
    (
        "cmdCompletions",
        [
            "Escreve o complemento com Tab para um shell",
            "Writes Tab completion for a shell",
            "Escribe el completado con Tab para un shell",
        ],
    ),
    (
        "argPaths",
        [
            "Arquivos e pastas (as pastas entram com as subpastas)",
            "Files and folders (folders come with their subfolders)",
            "Archivos y carpetas (las carpetas entran con sus subcarpetas)",
        ],
    ),
    (
        "argRule",
        [
            "Uma regra: TIPO:opção=valor,… (repetível, na ordem dada; veja renome-cli rules)",
            "A rule: TYPE:option=value,… (repeatable, in the order given; see renome-cli rules)",
            "Una regla: TIPO:opción=valor,… (repetible, en el orden dado; vea renome-cli rules)",
        ],
    ),
    (
        "argSet",
        [
            "Um conjunto de regras salvo, pelo nome",
            "A saved rule set, by name",
            "Un conjunto de reglas guardado, por nombre",
        ],
    ),
    (
        "argRules",
        [
            "Regras de um arquivo JSON, como a janela exporta (- lê da entrada)",
            "Rules from a JSON file, as the window exports them (- reads stdin)",
            "Reglas de un archivo JSON, como las exporta la ventana (- lee la entrada)",
        ],
    ),
    (
        "argSaved",
        [
            "As regras que a janela lembrou",
            "The rules the window remembered",
            "Las reglas que recordó la ventana",
        ],
    ),
    (
        "argFilter",
        [
            "Filtro da lista (nomes, >40MB, =2024, <7d, folder:…): só o que ele mostra é renomeado",
            "List filter (names, >40MB, =2024, <7d, folder:…): only what it shows is renamed",
            "Filtro de la lista (nombres, >40MB, =2024, <7d, folder:…): solo se renombra lo que muestra",
        ],
    ),
    (
        "argSort",
        [
            "Ordem dos arquivos (a numeração a segue)",
            "Order of the files (numbering follows it)",
            "Orden de los archivos (la numeración lo sigue)",
        ],
    ),
    (
        "argDesc",
        ["Ordem decrescente", "Descending order", "Orden descendente"],
    ),
    (
        "argHidden",
        [
            "Inclui arquivos ocultos",
            "Includes hidden files",
            "Incluye archivos ocultos",
        ],
    ),
    (
        "argNoRecursive",
        [
            "Sem as subpastas",
            "Without subfolders",
            "Sin las subcarpetas",
        ],
    ),
    (
        "argFolders",
        [
            "As pastas dadas entram elas mesmas, para renomeá-las",
            "The given folders themselves, to rename them",
            "Las carpetas dadas entran ellas mismas, para renombrarlas",
        ],
    ),
    (
        "argName",
        [
            "Dá um nome à mão: CAMINHO=NOME (repetível)",
            "Gives a name by hand: PATH=NAME (repeatable)",
            "Da un nombre a mano: RUTA=NOMBRE (repetible)",
        ],
    ),
    (
        "argTo",
        [
            "Move para esta pasta",
            "Moves into this folder",
            "Mueve a esta carpeta",
        ],
    ),
    (
        "argCopy",
        [
            "Com --to: copia em vez de mover",
            "With --to: copies instead of moving",
            "Con --to: copia en vez de mover",
        ],
    ),
    (
        "argYes",
        [
            "Não pergunta antes de renomear",
            "Does not ask before renaming",
            "No pregunta antes de renombrar",
        ],
    ),
    (
        "argStrict",
        [
            "Não renomeia nada se sobrar algum conflito",
            "Renames nothing when any conflict is left",
            "No renombra nada si queda algún conflicto",
        ],
    ),
    (
        "argAll",
        [
            "Lista também os arquivos que não mudam",
            "Also lists the files that do not change",
            "Lista también los archivos que no cambian",
        ],
    ),
    (
        "argCsv",
        [
            "Salva a lista num CSV (Exportar lista)",
            "Saves the list as CSV (Export list)",
            "Guarda la lista en un CSV (Exportar lista)",
        ],
    ),
    (
        "argJson",
        [
            "Saída em JSON, para scripts",
            "JSON output, for scripts",
            "Salida en JSON, para scripts",
        ],
    ),
    (
        "argLang",
        [
            "Idioma: pt, en ou es",
            "Language: pt, en or es",
            "Idioma: pt, en o es",
        ],
    ),
    (
        "argColor",
        [
            "Cores: auto, always ou never",
            "Colours: auto, always or never",
            "Colores: auto, always o never",
        ],
    ),
    (
        "argQuiet",
        ["Só os erros", "Errors only", "Solo los errores"],
    ),
    (
        "argId",
        [
            "ID da entrada (veja renome-cli history)",
            "Entry ID (see renome-cli history)",
            "ID de la entrada (vea renome-cli history)",
        ],
    ),
    (
        "argRuleType",
        ["Tipo de regra", "Rule type", "Tipo de regla"],
    ),
    (
        "argShell",
        [
            "bash, zsh, fish, powershell ou elvish",
            "bash, zsh, fish, powershell or elvish",
            "bash, zsh, fish, powershell o elvish",
        ],
    ),
    (
        "histList",
        [
            "Lista as entradas",
            "Lists the entries",
            "Lista las entradas",
        ],
    ),
    (
        "histShow",
        [
            "Mostra o que uma entrada renomeou",
            "Shows what an entry renamed",
            "Muestra lo que renombró una entrada",
        ],
    ),
    (
        "histDelete",
        [
            "Apaga uma entrada (os arquivos ficam como estão)",
            "Deletes an entry (files stay as they are)",
            "Borra una entrada (los archivos quedan como están)",
        ],
    ),
    (
        "histClear",
        [
            "Apaga todo o Histórico",
            "Deletes the whole History",
            "Borra todo el Historial",
        ],
    ),
    (
        "setsList",
        [
            "Lista os conjuntos",
            "Lists the sets",
            "Lista los conjuntos",
        ],
    ),
    (
        "setsShow",
        [
            "Mostra as regras de um conjunto",
            "Shows a set's rules",
            "Muestra las reglas de un conjunto",
        ],
    ),
    (
        "setsSave",
        [
            "Salva as regras dadas (-r, --set, --rules, --saved) como um conjunto",
            "Saves the given rules (-r, --set, --rules, --saved) as a set",
            "Guarda las reglas dadas (-r, --set, --rules, --saved) como un conjunto",
        ],
    ),
    (
        "setsExport",
        [
            "Escreve um conjunto num arquivo JSON (- na saída)",
            "Writes a set to a JSON file (- to stdout)",
            "Escribe un conjunto en un archivo JSON (- en la salida)",
        ],
    ),
    (
        "setsImport",
        [
            "Lê um conjunto de um arquivo JSON",
            "Reads a set from a JSON file",
            "Lee un conjunto de un archivo JSON",
        ],
    ),
    (
        "setsDelete",
        ["Apaga um conjunto", "Deletes a set", "Borra un conjunto"],
    ),
    (
        "setsRename",
        [
            "Renomeia um conjunto",
            "Renames a set",
            "Renombra un conjunto",
        ],
    ),
    (
        "argSetName",
        ["Nome do conjunto", "Set name", "Nombre del conjunto"],
    ),
    ("argNewName", ["Nome novo", "New name", "Nombre nuevo"]),
    ("argFile", ["Arquivo", "File", "Archivo"]),
    (
        "argReplace",
        [
            "Substitui um conjunto com o mesmo nome",
            "Replaces a set with the same name",
            "Reemplaza un conjunto con el mismo nombre",
        ],
    ),
    (
        "optList",
        [
            "Lista as opções e seus valores",
            "Lists the options and their values",
            "Lista las opciones y sus valores",
        ],
    ),
    (
        "optGet",
        [
            "Mostra o valor de uma opção",
            "Shows an option's value",
            "Muestra el valor de una opción",
        ],
    ),
    (
        "optSet",
        ["Muda uma opção", "Changes an option", "Cambia una opción"],
    ),
    (
        "optReset",
        [
            "Volta uma opção ao padrão",
            "Puts an option back to its default",
            "Devuelve una opción a su valor por defecto",
        ],
    ),
    (
        "argOption",
        ["Nome da opção", "Option name", "Nombre de la opción"],
    ),
    ("argValue", ["Valor", "Value", "Valor"]),
    (
        "keysList",
        [
            "Lista as chaves e onde estão (nunca os valores)",
            "Lists the keys and where they are (never their values)",
            "Lista las claves y dónde están (nunca sus valores)",
        ],
    ),
    (
        "keysSet",
        [
            "Guarda uma chave (digitada sem aparecer, ou da entrada com --stdin)",
            "Stores a key (typed without showing, or from stdin with --stdin)",
            "Guarda una clave (escrita sin mostrarse, o de la entrada con --stdin)",
        ],
    ),
    (
        "keysDelete",
        ["Apaga uma chave", "Deletes a key", "Borra una clave"],
    ),
    (
        "argKey",
        [
            "Nome da chave (veja renome-cli keys list)",
            "Key name (see renome-cli keys list)",
            "Nombre de la clave (vea renome-cli keys list)",
        ],
    ),
    (
        "argStdin",
        [
            "Lê o valor da entrada",
            "Reads the value from stdin",
            "Lee el valor de la entrada",
        ],
    ),
    (
        "cacheSize",
        ["Quanto ocupa", "How much space it takes", "Cuánto ocupa"],
    ),
    (
        "cacheClear",
        [
            "Apaga as respostas guardadas",
            "Deletes the kept answers",
            "Borra las respuestas guardadas",
        ],
    ),
    // Messages.
    (
        "noFiles",
        ["Nenhum arquivo.", "No files.", "Ningún archivo."],
    ),
    (
        "noRules",
        [
            "Nenhuma regra: use -r, --set, --rules ou --saved.",
            "No rules: use -r, --set, --rules or --saved.",
            "Ninguna regla: use -r, --set, --rules o --saved.",
        ],
    ),
    (
        "unknownRule",
        [
            "Regra desconhecida: {name} (veja renome-cli rules)",
            "Unknown rule: {name} (see renome-cli rules)",
            "Regla desconocida: {name} (vea renome-cli rules)",
        ],
    ),
    (
        "unknownOption",
        [
            "A regra {rule} não tem a opção {name} (veja renome-cli rules {rule})",
            "The {rule} rule has no option {name} (see renome-cli rules {rule})",
            "La regla {rule} no tiene la opción {name} (vea renome-cli rules {rule})",
        ],
    ),
    (
        "badValue",
        [
            "Valor inválido para {name}: {value} (esperado: {kind})",
            "Invalid value for {name}: {value} (expected: {kind})",
            "Valor inválido para {name}: {value} (se esperaba: {kind})",
        ],
    ),
    (
        "badSpec",
        [
            "Regra mal escrita: {spec} (use TIPO:opção=valor,…)",
            "Badly written rule: {spec} (use TYPE:option=value,…)",
            "Regla mal escrita: {spec} (use TIPO:opción=valor,…)",
        ],
    ),
    (
        "lookupLater",
        [
            "A regra {rule} busca em catálogos online, o que a linha de comando ainda não faz.",
            "The {rule} rule looks things up in online catalogues, which the command line does not do yet.",
            "La regla {rule} busca en catálogos en línea, lo que la línea de comandos aún no hace.",
        ],
    ),
    (
        "twoWhole",
        [
            "Só uma regra que faz o nome inteiro por vez: {a} e {b}.",
            "Only one rule that makes the whole name at a time: {a} and {b}.",
            "Solo una regla que hace el nombre entero a la vez: {a} y {b}.",
        ],
    ),
    (
        "noSet",
        [
            "Não há conjunto chamado {name}.",
            "There is no set called {name}.",
            "No hay ningún conjunto llamado {name}.",
        ],
    ),
    (
        "setExists",
        [
            "Já existe um conjunto chamado {name} (use --replace).",
            "A set called {name} already exists (use --replace).",
            "Ya existe un conjunto llamado {name} (use --replace).",
        ],
    ),
    (
        "setSaved",
        [
            "Conjunto {name} salvo, com {n} regra(s).",
            "Set {name} saved, with {n} rule(s).",
            "Conjunto {name} guardado, con {n} regla(s).",
        ],
    ),
    (
        "setDeleted",
        [
            "Conjunto {name} apagado.",
            "Set {name} deleted.",
            "Conjunto {name} borrado.",
        ],
    ),
    (
        "setRenamed",
        [
            "Conjunto {from} agora se chama {to}.",
            "Set {from} is now called {to}.",
            "El conjunto {from} ahora se llama {to}.",
        ],
    ),
    (
        "noSaved",
        [
            "A janela não lembrou nenhuma regra.",
            "The window remembered no rules.",
            "La ventana no recordó ninguna regla.",
        ],
    ),
    (
        "badFile",
        [
            "Não foi possível ler {file}: {error}",
            "Could not read {file}: {error}",
            "No se pudo leer {file}: {error}",
        ],
    ),
    (
        "badName",
        [
            "Nome à mão mal escrito: {spec} (use CAMINHO=NOME)",
            "Badly written name: {spec} (use PATH=NAME)",
            "Nombre a mano mal escrito: {spec} (use RUTA=NOMBRE)",
        ],
    ),
    (
        "askRename",
        ["Renomear? [s/N] ", "Rename? [y/N] ", "¿Renombrar? [s/N] "],
    ),
    (
        "needYes",
        [
            "Sem terminal para perguntar: use --yes para renomear.",
            "No terminal to ask: use --yes to rename.",
            "Sin terminal para preguntar: use --yes para renombrar.",
        ],
    ),
    (
        "cancelled",
        [
            "Nada foi renomeado.",
            "Nothing was renamed.",
            "No se renombró nada.",
        ],
    ),
    (
        "strictConflicts",
        [
            "{n} conflito(s): nada foi renomeado (--strict).",
            "{n} conflict(s): nothing was renamed (--strict).",
            "{n} conflicto(s): no se renombró nada (--strict).",
        ],
    ),
    (
        "waitingWindow",
        [
            "Esperando a janela do Renome terminar de renomear…",
            "Waiting for the Renome window to finish renaming…",
            "Esperando a que la ventana de Renome termine de renombrar…",
        ],
    ),
    (
        "historyEmpty",
        [
            "O Histórico está vazio.",
            "The History is empty.",
            "El Historial está vacío.",
        ],
    ),
    (
        "historyCleared",
        [
            "{n} entrada(s) apagada(s).",
            "{n} entr[y|ies] deleted.",
            "{n} entrada(s) borrada(s).",
        ],
    ),
    (
        "entryDeleted",
        [
            "Entrada {id} apagada.",
            "Entry {id} deleted.",
            "Entrada {id} borrada.",
        ],
    ),
    (
        "noEntry",
        [
            "Não há entrada {id} no Histórico.",
            "There is no entry {id} in the History.",
            "No hay ninguna entrada {id} en el Historial.",
        ],
    ),
    (
        "undoneN",
        [
            "{n} arquivo(s) de volta ao nome antigo.",
            "{n} file(s) back to their old name.",
            "{n} archivo(s) de vuelta a su nombre anterior.",
        ],
    ),
    (
        "recovered",
        [
            "Uma renomeação tinha ficado pela metade: {restored} arquivo(s) [voltou|voltaram] ao nome antigo, {renamed} [ficou|ficaram] renomeado(s) no Histórico.",
            "A rename had been left halfway: {restored} file(s) got their old name back, {renamed} stayed renamed in the History.",
            "Un renombrado había quedado a medias: {restored} archivo(s) [recuperó|recuperaron] su nombre, {renamed} [quedó|quedaron] renombrado(s) en el Historial.",
        ],
    ),
    (
        "unknownSetting",
        [
            "Opção desconhecida: {name} (veja renome-cli options list)",
            "Unknown option: {name} (see renome-cli options list)",
            "Opción desconocida: {name} (vea renome-cli options list)",
        ],
    ),
    (
        "windowOnly",
        ["só da janela", "window only", "solo de la ventana"],
    ),
    (
        "settingSaved",
        [
            "{name} = {value}. Com a janela aberta, ela pode gravar por cima ao fechar.",
            "{name} = {value}. With the window open, it may write over this when it closes.",
            "{name} = {value}. Con la ventana abierta, puede escribir encima al cerrarse.",
        ],
    ),
    (
        "unknownKey",
        [
            "Chave desconhecida: {name} (veja renome-cli keys list)",
            "Unknown key: {name} (see renome-cli keys list)",
            "Clave desconocida: {name} (vea renome-cli keys list)",
        ],
    ),
    (
        "keyPrompt",
        [
            "Valor de {name} (não aparece): ",
            "Value of {name} (not shown): ",
            "Valor de {name} (no se muestra): ",
        ],
    ),
    (
        "keyStored",
        [
            "{name} guardada no chaveiro do sistema.",
            "{name} stored in the system's password store.",
            "{name} guardada en el almacén de contraseñas del sistema.",
        ],
    ),
    (
        "keyInFile",
        [
            "{name} guardada no arquivo de opções (o sistema não tem chaveiro).",
            "{name} stored in the options file (the system has no password store).",
            "{name} guardada en el archivo de opciones (el sistema no tiene almacén de contraseñas).",
        ],
    ),
    (
        "keyStoreWouldNot",
        [
            "{name} guardada no arquivo de opções: o chaveiro do sistema não aceitou gravar (trancado, ou acessado por SSH?).",
            "{name} stored in the options file: the system's password store would not save it (locked, or reached over SSH?).",
            "{name} guardada en el archivo de opciones: el almacén de contraseñas del sistema no aceptó guardarla (¿bloqueado, o por SSH?).",
        ],
    ),
    (
        "keyDeleted",
        ["{name} apagada.", "{name} deleted.", "{name} borrada."],
    ),
    ("keySetHere", ["definida", "set", "definida"]),
    ("keyUnset", ["não definida", "not set", "no definida"]),
    (
        "keyWhereStore",
        [
            "no chaveiro do sistema",
            "in the system's password store",
            "en el almacén de contraseñas del sistema",
        ],
    ),
    (
        "keyWhereFile",
        [
            "no arquivo de opções",
            "in the options file",
            "en el archivo de opciones",
        ],
    ),
    (
        "keyStoreRefused",
        [
            "O chaveiro do sistema não respondeu (está trancado?).",
            "The system's password store did not answer (is it locked?).",
            "El almacén de contraseñas del sistema no respondió (¿está bloqueado?).",
        ],
    ),
    (
        "cacheSizeIs",
        [
            "As respostas guardadas ocupam {size}.",
            "The kept answers take {size}.",
            "Las respuestas guardadas ocupan {size}.",
        ],
    ),
    (
        "cacheCleared",
        [
            "Respostas guardadas apagadas.",
            "Kept answers deleted.",
            "Respuestas guardadas borradas.",
        ],
    ),
    (
        "rulesTitle",
        [
            "Regras (renome-cli rules TIPO mostra as opções):",
            "Rules (renome-cli rules TYPE shows the options):",
            "Reglas (renome-cli rules TIPO muestra las opciones):",
        ],
    ),
    ("ruleOptions", ["Opções", "Options", "Opciones"]),
    ("ruleExample", ["Exemplo", "Example", "Ejemplo"]),
    (
        "everyRule",
        [
            "Toda regra aceita também only=FILTRO (só nos arquivos) e off (desligada).",
            "Every rule also takes only=FILTER (only in files) and off (turned off).",
            "Toda regla acepta también only=FILTRO (solo en archivos) y off (desactivada).",
        ],
    ),
    ("wasNamed", ["antes", "formerly", "antes"]),
    ("kindText", ["texto", "text", "texto"]),
    (
        "kindTemplate",
        [
            "modelo com {chaves}",
            "template with {keys}",
            "plantilla con {claves}",
        ],
    ),
    (
        "kindLines",
        [
            "texto, um por linha (\\n)",
            "text, one per line (\\n)",
            "texto, uno por línea (\\n)",
        ],
    ),
    ("kindBool", ["sim/não", "yes/no", "sí/no"]),
    (
        "kindInt",
        ["número inteiro", "whole number", "número entero"],
    ),
    ("kindFloat", ["número", "number", "número"]),
    ("kindChoice", ["um de", "one of", "uno de"]),
    ("defaultIs", ["padrão", "default", "por defecto"]),
    (
        "unchangedN",
        ["{n} sem mudança", "{n} unchanged", "{n} sin cambio"],
    ),
    ("error", ["Erro", "Error", "Error"]),
    (
        "chooseKeys",
        [
            "Número (Enter = 1), s pula, /texto busca, q para, ? ajuda:",
            "Number (Enter = 1), s skips, /text searches, q stops, ? help:",
            "Número (Enter = 1), s omite, /texto busca, q para, ? ayuda:",
        ],
    ),
    (
        "chooseHelp",
        [
            "  1-9 escolhe · Enter escolhe o primeiro · s deixa este arquivo como está · /título 1999 busca de novo (ou cole um link ou ID) · q para tudo, sem renomear nada",
            "  1-9 picks · Enter picks the first · s leaves this file as it is · /title 1999 searches again (or paste a link or ID) · q stops everything, renaming nothing",
            "  1-9 elige · Enter elige el primero · s deja este archivo como está · /título 1999 busca de nuevo (o pegue un enlace o ID) · q lo para todo, sin renombrar nada",
        ],
    ),
    (
        "argChoose",
        [
            "Casos duvidosos das buscas: ask (pergunta no terminal), auto (só o que a janela escolheria sozinha), skip",
            "Doubtful lookups: ask (in the terminal), auto (only what the window would pick by itself), skip",
            "Casos dudosos de las búsquedas: ask (pregunta en el terminal), auto (solo lo que la ventana elegiría sola), skip",
        ],
    ),
    (
        "argPick",
        [
            "Força uma escolha: CAMINHO=ID (tmdb:603, um link ou um título)",
            "Forces a match: PATH=ID (tmdb:603, a link or a title)",
            "Fuerza una elección: RUTA=ID (tmdb:603, un enlace o un título)",
        ],
    ),
    (
        "argMissing",
        [
            "Lista também o que falta: episódios, edições, faixas, filmes da coleção, volumes",
            "Also lists what is missing: episodes, issues, tracks, collection movies, volumes",
            "Lista también lo que falta: episodios, números, pistas, películas de la colección, volúmenes",
        ],
    ),
    ("missingTitle", ["Faltando", "Missing", "Faltan"]),
    (
        "argMissingCsv",
        [
            "Salva o que falta num CSV (Exportar o que falta); com --missing implícito",
            "Saves what is missing as CSV (Export what is missing); implies --missing",
            "Guarda lo que falta en un CSV (Exportar lo que falta); implica --missing",
        ],
    ),
    (
        "cmdServers",
        [
            "Servidores de mídia (Jellyfin, Emby, Plex) avisados das pastas que mudam",
            "Media servers (Jellyfin, Emby, Plex) told about the folders that change",
            "Servidores multimedia (Jellyfin, Emby, Plex) avisados de las carpetas que cambian",
        ],
    ),
    (
        "srvList",
        [
            "Lista os servidores e suas pastas",
            "Lists the servers and their folders",
            "Lista los servidores y sus carpetas",
        ],
    ),
    (
        "srvAdd",
        [
            "Adiciona um servidor (pede a chave) e lê as bibliotecas",
            "Adds a server (asks for its key) and reads its libraries",
            "Añade un servidor (pide la clave) y lee las bibliotecas",
        ],
    ),
    (
        "srvRemove",
        [
            "Remove um servidor",
            "Removes a server",
            "Quita un servidor",
        ],
    ),
    (
        "srvLibraries",
        [
            "Lê de novo as bibliotecas (os pares ficam)",
            "Reads the libraries again (pairs stay)",
            "Lee de nuevo las bibliotecas (los pares se quedan)",
        ],
    ),
    (
        "srvPair",
        [
            "Pareia uma pasta do servidor com a mesma pasta neste computador",
            "Pairs a server folder with the same folder on this computer",
            "Empareja una carpeta del servidor con la misma carpeta en este equipo",
        ],
    ),
    (
        "srvOn",
        [
            "Avisa sobre esta pasta",
            "Tells about this folder",
            "Avisa sobre esta carpeta",
        ],
    ),
    (
        "srvOff",
        [
            "Não avisa sobre esta pasta",
            "Does not tell about this folder",
            "No avisa sobre esta carpeta",
        ],
    ),
    (
        "srvTest",
        [
            "Testa o endereço e a chave",
            "Tests the address and key",
            "Prueba la dirección y la clave",
        ],
    ),
    (
        "argServerId",
        [
            "ID do servidor (veja renome-cli servers list)",
            "Server ID (see renome-cli servers list)",
            "ID del servidor (vea renome-cli servers list)",
        ],
    ),
    (
        "argServerPath",
        [
            "A pasta como o servidor a vê",
            "The folder as the server sees it",
            "La carpeta como la ve el servidor",
        ],
    ),
    (
        "argServerKind",
        [
            "jellyfin (também Emby) ou plex",
            "jellyfin (Emby too) or plex",
            "jellyfin (también Emby) o plex",
        ],
    ),
    (
        "argServerUrl",
        [
            "Endereço, como http://192.168.0.10:8096",
            "Address, like http://192.168.0.10:8096",
            "Dirección, como http://192.168.0.10:8096",
        ],
    ),
    (
        "argLocalPath",
        [
            "A mesma pasta neste computador",
            "The same folder on this computer",
            "La misma carpeta en este equipo",
        ],
    ),
    (
        "argNoServer",
        [
            "Não avisa os servidores de mídia",
            "Does not tell the media servers",
            "No avisa a los servidores multimedia",
        ],
    ),
    (
        "srvUnpaired",
        [
            "sem pasta neste computador (renome-cli servers pair)",
            "no folder on this computer (renome-cli servers pair)",
            "sin carpeta en este equipo (renome-cli servers pair)",
        ],
    ),
    (
        "noServer",
        [
            "Não há servidor {id}.",
            "There is no server {id}.",
            "No hay ningún servidor {id}.",
        ],
    ),
    (
        "noServerFolder",
        [
            "O servidor não tem a pasta {path}.",
            "The server has no folder {path}.",
            "El servidor no tiene la carpeta {path}.",
        ],
    ),
    (
        "serverRemoved",
        [
            "Servidor {id} removido.",
            "Server {id} removed.",
            "Servidor {id} quitado.",
        ],
    ),
    (
        "serverOk",
        [
            "O servidor respondeu.",
            "The server answered.",
            "El servidor respondió.",
        ],
    ),
    (
        "cmdDuplicates",
        [
            "Cópias idênticas, a mesma música ou obra repetida, fotos parecidas",
            "Identical copies, the same song or work twice, pictures that look alike",
            "Copias idénticas, la misma canción u obra repetida, fotos parecidas",
        ],
    ),
    (
        "argIdentical",
        [
            "Cópias idênticas (byte a byte)",
            "Identical copies (byte for byte)",
            "Copias idénticas (byte a byte)",
        ],
    ),
    (
        "argRepeated",
        [
            "A mesma música, filme, episódio, quadrinho ou livro em vários arquivos (a melhor marcada com ★)",
            "The same song, movie, episode, comic or book in several files (the best marked ★)",
            "La misma canción, película, episodio, cómic o libro en varios archivos (la mejor marcada con ★)",
        ],
    ),
    (
        "argSimilar",
        [
            "Fotos parecidas (mais lento)",
            "Pictures that look alike (slower)",
            "Fotos parecidas (más lento)",
        ],
    ),
    (
        "argTrash",
        [
            "Manda estes arquivos para a Lixeira (sem desfazer pelo Histórico)",
            "Sends these files to the Trash (not undone from the History)",
            "Envía estos archivos a la Papelera (sin deshacer desde el Historial)",
        ],
    ),
    (
        "cmdUpdate",
        [
            "Procura (e instala) a versão nova do Renome",
            "Looks for (and installs) a new version of Renome",
            "Busca (e instala) la versión nueva de Renome",
        ],
    ),
    ("updCheck", ["Só procura", "Only looks", "Solo busca"]),
    (
        "updInstall",
        [
            "Baixa, confere a assinatura e instala (AppImage, Windows, macOS)",
            "Downloads, checks the signature and installs (AppImage, Windows, macOS)",
            "Descarga, comprueba la firma e instala (AppImage, Windows, macOS)",
        ],
    ),
    (
        "upToDate",
        [
            "O Renome {v} é a versão mais nova.",
            "Renome {v} is the newest version.",
            "Renome {v} es la versión más nueva.",
        ],
    ),
    (
        "updateByFlatpak",
        [
            "No Flatpak, o próprio Flatpak atualiza o Renome.",
            "In the Flatpak, Flatpak updates Renome.",
            "En el Flatpak, el propio Flatpak actualiza Renome.",
        ],
    ),
    (
        "updateHowCli",
        [
            "Para instalar: renome-cli update install",
            "To install it: renome-cli update install",
            "Para instalarla: renome-cli update install",
        ],
    ),
    (
        "updateInstalled",
        [
            "Renome {v} instalado. A janela, se estiver aberta, usa a versão nova quando for aberta de novo.",
            "Renome {v} installed. The window, if open, uses the new version when opened again.",
            "Renome {v} instalado. La ventana, si está abierta, usa la versión nueva al abrirse de nuevo.",
        ],
    ),
    (
        "cmdManual",
        [
            "Mostra o manual (o mesmo da janela)",
            "Shows the manual (the window's)",
            "Muestra el manual (el de la ventana)",
        ],
    ),
    (
        "argTopic",
        [
            "Capítulo: o código (movie, series…) ou o começo do título; sem ele, o índice",
            "Chapter: its code (movie, series…) or the start of its title; without it, the index",
            "Capítulo: su código (movie, series…) o el comienzo del título; sin él, el índice",
        ],
    ),
    (
        "argManualAll",
        ["O manual inteiro", "The whole manual", "El manual entero"],
    ),
    (
        "manualHow",
        [
            "Para ler um capítulo: renome-cli manual CÓDIGO",
            "To read a chapter: renome-cli manual CODE",
            "Para leer un capítulo: renome-cli manual CÓDIGO",
        ],
    ),
    (
        "noManualTopic",
        [
            "O manual não tem o capítulo \"{topic}\".",
            "The manual has no chapter \"{topic}\".",
            "El manual no tiene el capítulo \"{topic}\".",
        ],
    ),
    (
        "argEdit",
        [
            "Corrige à mão os detalhes de um arquivo, repetível. Campos: música {music}; livro {book}; quadrinho {comic}; foto {photo}",
            "Corrects a file's details by hand, repeatable. Fields: music {music}; book {book}; comic {comic}; photo {photo}",
            "Corrige a mano los detalles de un archivo, repetible. Campos: música {music}; libro {book}; cómic {comic}; foto {photo}",
        ],
    ),
    (
        "badEdit",
        [
            "--edit {spec}: escreva CAMINHO:CAMPO=VALOR (campos: {keys}).",
            "--edit {spec}: write PATH:FIELD=VALUE (fields: {keys}).",
            "--edit {spec}: escriba RUTA:CAMPO=VALOR (campos: {keys}).",
        ],
    ),
    (
        "badEditNumber",
        [
            "--edit: {key} leva só algarismos, não \"{value}\".",
            "--edit: {key} takes digits only, not \"{value}\".",
            "--edit: {key} lleva solo dígitos, no \"{value}\".",
        ],
    ),
    (
        "noEditor",
        [
            "--edit precisa de uma regra de música, livro, quadrinho ou foto.",
            "--edit needs a music, book, comic or photo rule.",
            "--edit necesita una regla de música, libro, cómic o foto.",
        ],
    ),
    (
        "keysTest",
        [
            "Testa uma chave ou conta, como o botão Testar das Opções",
            "Tests a key or account, as the Options' Test button",
            "Prueba una clave o cuenta, como el botón Probar de las Opciones",
        ],
    ),
    (
        "keyUnsetTest",
        [
            "{name} não está definida (renome-cli keys set {name}).",
            "{name} is not set (renome-cli keys set {name}).",
            "{name} no está definida (renome-cli keys set {name}).",
        ],
    ),
    (
        "stopping",
        [
            "Parando… (Ctrl+C de novo encerra)",
            "Stopping… (Ctrl+C again quits)",
            "Deteniendo… (Ctrl+C de nuevo termina)",
        ],
    ),
    (
        "goOnAfterStop",
        [
            "Busca interrompida. Continuar com o que foi achado?",
            "Search stopped. Go on with what was found?",
            "Búsqueda detenida. ¿Seguir con lo encontrado?",
        ],
    ),
    (
        "stoppedRenaming",
        [
            "Renomeação interrompida: o que já foi renomeado está no Histórico (renome-cli undo desfaz); tags, legendas e o resto não foram feitos.",
            "Renaming stopped: what was renamed is in the History (renome-cli undo undoes it); tags, subtitles and the rest were not done.",
            "Renombrado detenido: lo ya renombrado está en el Historial (renome-cli undo lo deshace); etiquetas, subtítulos y lo demás no se hicieron.",
        ],
    ),
    ("helpUsage", ["Uso", "Usage", "Uso"]),
    ("helpCommands", ["Comandos", "Commands", "Comandos"]),
    ("helpCommand", ["comando", "command", "comando"]),
    ("helpArgs", ["Argumentos", "Arguments", "Argumentos"]),
    ("helpOptions", ["Opções", "Options", "Opciones"]),
    (
        "helpHelp",
        ["Mostra esta ajuda", "Shows this help", "Muestra esta ayuda"],
    ),
    (
        "helpVersion",
        ["Mostra a versão", "Shows the version", "Muestra la versión"],
    ),
    (
        "helpHelpCmd",
        [
            "Mostra a ajuda de um comando",
            "Shows a command's help",
            "Muestra la ayuda de un comando",
        ],
    ),
    (
        "epTypeHint",
        [
            "digite S01E05 (ou 1x05, S01E05-06) para escolher outro",
            "type S01E05 (or 1x05, S01E05-06) to pick another",
            "escriba S01E05 (o 1x05, S01E05-06) para elegir otro",
        ],
    ),
    (
        "epTypeBad",
        [
            "{text} não é um episódio desta série (use S01E05)",
            "{text} is not an episode of this show (use S01E05)",
            "{text} no es un episodio de esta serie (use S01E05)",
        ],
    ),
    (
        "badPick",
        [
            "Escolha mal escrita: {spec} (use CAMINHO=ID)",
            "Badly written pick: {spec} (use PATH=ID)",
            "Elección mal escrita: {spec} (use RUTA=ID)",
        ],
    ),
    (
        "leftDoubtful",
        [
            "{n} arquivo(s) com dúvida [ficou|ficaram] como [estava|estavam] (use --choose ask ou --pick).",
            "{n} doubtful file(s) left as they were (use --choose ask or --pick).",
            "{n} archivo(s) con duda [quedó|quedaron] como [estaba|estaban] (use --choose ask o --pick).",
        ],
    ),
];

/// A text by its key: the command line's own, else the page's, in the
/// language in use (Portuguese, then English when missing); the key when
/// neither has it.
pub fn raw(key: &str) -> String {
    let i = match lang() {
        "pt" => 0,
        "es" => 2,
        _ => 1,
    };
    if let Some((_, by)) = CLI.iter().find(|(k, _)| *k == key) {
        return by[i].to_string();
    }
    let dict = match i {
        0 => page::PT,
        2 => page::ES,
        _ => page::EN,
    };
    for d in [dict, page::EN, page::PT] {
        if let Ok(at) = d.binary_search_by(|(k, _)| (*k).cmp(key)) {
            return d[at].1.to_string();
        }
    }
    key.to_string()
}

/// Whether the page or the command line has this text.
#[cfg(test)]
pub fn has(key: &str) -> bool {
    raw(key) != key
}

/// A text with its `{vars}` filled in and its counts made singular or
/// plural.
pub fn t(key: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    fill(&raw(key), vars)
}

/// [`t`] on a text given directly.
pub fn fill(text: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    let vars: Vec<(&str, String)> = vars.iter().map(|(k, v)| (*k, v.to_string())).collect();
    let text = plurals(text, &vars);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close)
                if after[..close]
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_') =>
            {
                let name = &after[..close];
                match vars.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => out.push_str(&rest[open..open + close + 2]),
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Words whose plural is not the singular plus the letters in brackets
/// (the page's table, `src/lib/plurals.json`).
static IRREGULAR: std::sync::LazyLock<Vec<(String, String)>> = std::sync::LazyLock::new(|| {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/lib/plurals.json")).unwrap_or_default();
    v["irregular"]
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect()
        })
        .unwrap_or_default()
});

const ENDINGS: &[&str] = &["s", "es", "ns", "ões", "eis", "ies", "m", "n"];

/// As the page's `plurals`: each `word(s)` and `[one|many]` follows the
/// number before it (the first in the text when none comes before);
/// without numbers, left as is.
fn plurals(text: &str, vars: &[(&str, String)]) -> String {
    // Where each count var stands, and its number.
    let mut numbers: Vec<(usize, i64)> = Vec::new();
    let mut at = 0;
    while let Some(open) = text[at..].find('{') {
        let start = at + open;
        let Some(close) = text[start..].find('}') else {
            break;
        };
        let name = &text[start + 1..start + close];
        if let Some((_, v)) = vars.iter().find(|(k, _)| *k == name)
            && let Ok(n) = v.parse::<i64>()
        {
            numbers.push((start, n));
        }
        at = start + close + 1;
    }
    if numbers.is_empty() {
        return text.to_string();
    }
    let count_at = |pos: usize| {
        numbers
            .iter()
            .rev()
            .find(|(a, _)| *a < pos)
            .unwrap_or(&numbers[0])
            .1
    };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let c = text[i..].chars().next().unwrap();
        if c == '['
            && let Some(close) = text[i..].find(']')
            && let Some((one, many)) = text[i + 1..i + close].split_once('|')
            && !one.contains('[')
            && !many.contains('|')
        {
            out.push_str(if count_at(i) == 1 { one } else { many });
            i += close + 1;
            continue;
        }
        if c == '(' {
            let rest = &text[i + 1..];
            if let Some(close) = rest.find(')') {
                let end = &rest[..close];
                if ENDINGS.contains(&end) {
                    // The word before, already written.
                    let word_start = out
                        .char_indices()
                        .rev()
                        .take_while(|(_, ch)| ch.is_alphabetic())
                        .last()
                        .map_or(out.len(), |(p, _)| p);
                    let word = out[word_start..].to_string();
                    if count_at(i) != 1 {
                        let whole = format!("{word}({end})").to_lowercase();
                        if let Some((_, plural)) = IRREGULAR.iter().find(|(w, _)| *w == whole) {
                            out.truncate(word_start);
                            out.push_str(plural);
                        } else {
                            out.push_str(end);
                        }
                    }
                    i += close + 2;
                    continue;
                }
            }
        }
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// An error from the shared code, `code` or `code: detail`, in words (as
/// the page's `tError`).
pub fn error_text(raw: &str) -> String {
    let (code, detail) = raw.split_once(": ").unwrap_or((raw, ""));
    if raw_has(code) {
        let text = raw_text(code);
        if detail.is_empty() {
            text
        } else {
            format!("{text}: {detail}")
        }
    } else {
        raw.to_string()
    }
}

fn raw_has(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') && raw(key) != key
}

fn raw_text(key: &str) -> String {
    raw(key)
}

#[cfg(test)]
mod tests {

    #[test]
    fn plurals_as_the_page() {
        // The cases the window's plurals run too (scripts/plurals-check.ts).
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../../src/lib/plurals.json")).unwrap();
        for c in v["cases"].as_array().unwrap() {
            let vars: Vec<(&str, String)> = c["vars"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str(),
                        v.as_str().map_or_else(|| v.to_string(), str::to_string),
                    )
                })
                .collect();
            let text = c["text"].as_str().unwrap();
            assert_eq!(
                super::plurals(text, &vars),
                c["out"].as_str().unwrap(),
                "{text}"
            );
        }
    }

    use super::*;

    #[test]
    fn counts_agree() {
        let one: &[(&str, &dyn std::fmt::Display)] = &[("n", &1)];
        let three: &[(&str, &dyn std::fmt::Display)] = &[("n", &3)];
        assert_eq!(
            fill("{n} arquivo(s) que já [tem|têm]", one),
            "1 arquivo que já tem"
        );
        assert_eq!(
            fill("{n} arquivo(s) que já [tem|têm]", three),
            "3 arquivos que já têm"
        );
        assert_eq!(fill("{n} álbum(ns)", three), "3 álbuns");
        assert_eq!(fill("{n} edição(ões)", one), "1 edição");
        assert_eq!(
            fill("Se [descargará|descargarán] {n} subtítulo(s).", one),
            "Se descargará 1 subtítulo."
        );
        assert_eq!(fill("{x} literal {y}", &[("x", &"a")]), "a literal {y}");
    }

    #[test]
    fn page_texts_are_read() {
        let _turn = TEST_LANG.lock().unwrap_or_else(|e| e.into_inner());
        set_lang("pt");
        assert_eq!(raw("rule.replace"), "Substituir texto");
        assert!(has("renamedN"));
        assert_eq!(t("renamedN", &[("n", &2)]), "2 arquivos renomeados");
    }
}
