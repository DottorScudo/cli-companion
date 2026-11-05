//! Test architetturale: il core non nomina mai un connettore specifico. Tutto cio'
//! che e' proprio di un connettore (id, moduli, host, API del servizio) sta in
//! `connectors/<cartella>/`; fuori da li' si passa dai contratti. Fa eccezione solo
//! la registrazione (`connectors.rs` dichiara i moduli, `connectors/registry.rs` li
//! elenca). Inoltre un connettore non usa `reqwest` (solo il client del contesto) e
//! non importa `hub::` se non `hub::ingress` e i tipi pubblici del modello.
//!
//! Si leggono i soli identificatori e stringhe del codice di produzione (lo scanner
//! di `agents/architecture/scanner.rs`): restano fuori commenti, test e moduli
//! dichiarati `#[cfg(test)] mod x;`. L'elenco dei connettori
//! noti coincide con il registro di produzione; i casi sintetici provano le regole.

use super::registry;
use crate::agents::architecture::scanner::{scan, Token, TokenKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Un connettore e cio' che lo identifica nel codice.
struct Known {
    /// Cartella (e modulo) sotto `connectors/`.
    folder: &'static str,
    /// Id del descrittore: nessuna stringa fuori dalla cartella lo contiene.
    id: &'static str,
}

/// Connettori noti al test. Aggiungendo un connettore: una riga qui (e una in
/// `tests/architecture.test.mjs`).
const KNOWN: &[Known] = &[Known { folder: "telegram", id: "telegram" }];

/// Cartelle di `connectors/` che non sono connettori.
const CORE_FOLDERS: [&str; 8] =
    ["commands", "contracts", "descriptor", "files", "model", "registry", "runtime", "testing"];

/// File che dichiarano e registrano i moduli dei connettori.
const REGISTRATION_FILES: [&str; 2] = ["connectors.rs", "connectors/registry.rs"];

/// Cosa un connettore puo' importare da `crate::hub::`: la porta d'ingresso e i
/// tipi pubblici del modello (autore, messaggio, origine, canale, allegati in arrivo).
const ALLOWED_HUB: [&str; 8] =
    ["ingress", "attachments", "Author", "Message", "MessageOrigin", "Channel", "HubNotice", "NoticeCategory"];

/// I moduli di `hub` che un connettore non deve toccare nemmeno dentro un
/// `use crate::hub::{...}` (dove si vede solo la prima voce).
const FORBIDDEN_HUB: [&str; 12] = [
    "send",
    "engine",
    "settings",
    "commands",
    "state",
    "notify",
    "threads",
    "events",
    "files",
    "actions",
    "lifecycle",
    "channel",
];

/// Parole che aprono un nuovo elemento: chiudono la finestra di un `use`.
const ITEM_KEYWORDS: [&str; 14] =
    ["use", "pub", "fn", "struct", "enum", "impl", "trait", "const", "static", "type", "mod", "let", "async", "extern"];

/// Un riferimento a un connettore (o un'importazione vietata) in un punto in cui non dovrebbe esserci.
#[derive(Debug, PartialEq, Eq)]
struct Violation {
    file: String,
    line: usize,
    what: String,
}

/// La cartella `connectors/<x>/` di un file, se e' dentro una sotto-cartella.
fn connector_folder_of(path: &str) -> Option<&str> {
    path.strip_prefix("connectors/")?.split('/').next().filter(|_| path.matches('/').count() >= 2)
}

fn is_connector_folder(folder: &str) -> bool {
    !CORE_FOLDERS.contains(&folder)
}

fn ident_at(tokens: &[Token], index: usize) -> Option<&str> {
    tokens.get(index).filter(|t| t.kind == TokenKind::Ident).map(|t| t.text.as_str())
}

/// Riferimenti a connettori nei token del file `path` (relativo a `src/`, slash
/// dritti), esclusi quelli della cartella di ciascun connettore.
fn naming_violations(path: &str, tokens: &[Token], known: &[Known]) -> Vec<Violation> {
    let home = connector_folder_of(path);
    let may_name_modules = REGISTRATION_FILES.contains(&path);
    let mut found = Vec::new();
    for connector in known.iter().filter(|c| home != Some(c.folder)) {
        for token in tokens {
            let what = match token.kind {
                TokenKind::Ident if token.text == connector.folder && !may_name_modules => {
                    format!("il modulo `{}`", connector.folder)
                }
                TokenKind::Str if token.text.contains(connector.id) => {
                    format!("la stringa {:?} (id di `{}`)", token.text, connector.folder)
                }
                _ => continue,
            };
            found.push(Violation { file: path.to_string(), line: token.line, what });
        }
    }
    found
}

/// Regole per il codice dentro la cartella di un connettore: niente `reqwest`
/// (il client e' del contesto) e `crate::hub::` solo per cio' che e' ammesso.
fn connector_code_violations(path: &str, tokens: &[Token]) -> Vec<Violation> {
    if !connector_folder_of(path).is_some_and(is_connector_folder) {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut report = |line: usize, what: String| found.push(Violation { file: path.to_string(), line, what });
    for (i, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Ident {
            continue;
        }
        if token.text == "reqwest" {
            report(token.line, "`reqwest`: un connettore usa il client del contesto (`ctx.http()`)".to_string());
        }
        if token.text != "hub" || ident_at(tokens, i.wrapping_sub(1)) != Some("crate") {
            continue;
        }
        match ident_at(tokens, i + 1) {
            Some(first) if !ALLOWED_HUB.contains(&first) => {
                report(token.line, format!("`crate::hub::{first}`: un connettore importa da hub solo {ALLOWED_HUB:?}"));
            }
            Some(_) => {}
            None => report(token.line, "`crate::hub::` senza un elemento esplicito".to_string()),
        }
        // `use crate::hub::{...}`: dopo la prima voce si vedono solo gli identificatori.
        if ident_at(tokens, i.wrapping_sub(2)) == Some("use") {
            let window = tokens
                .get(i + 2..)
                .unwrap_or_default()
                .iter()
                .take_while(|t| !ITEM_KEYWORDS.contains(&t.text.as_str()));
            for inner in window.filter(|t| t.kind == TokenKind::Ident && FORBIDDEN_HUB.contains(&t.text.as_str())) {
                report(inner.line, format!("`hub::{}` in una importazione a gruppo", inner.text));
            }
        }
    }
    found
}

fn violations_in(path: &str, tokens: &[Token], known: &[Known]) -> Vec<Violation> {
    let mut found = naming_violations(path, tokens, known);
    found.extend(connector_code_violations(path, tokens));
    found
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cartella illeggibile {}: {e}", dir.display()));
    let mut paths: Vec<_> = entries.map(|e| e.expect("voce di cartella").path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// File che `file` dichiara come moduli di test (`#[cfg(test)] mod nome;`).
fn test_module_files(file: &Path, modules: &[String]) -> Vec<PathBuf> {
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let parent = file.parent().unwrap_or(Path::new(""));
    let base = if matches!(stem, "mod" | "lib" | "main") { parent.to_path_buf() } else { parent.join(stem) };
    modules.iter().flat_map(|m| [base.join(format!("{m}.rs")), base.join(m).join("mod.rs")]).collect()
}

/// Cartella dei sotto-moduli di un modulo di test: anche i suoi file sono di test.
fn test_module_dir(file: &Path) -> PathBuf {
    if file.file_name().is_some_and(|n| n == "mod.rs") {
        file.parent().unwrap_or(Path::new("")).to_path_buf()
    } else {
        file.with_extension("")
    }
}

fn relative(src: &Path, file: &Path) -> String {
    file.strip_prefix(src).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// Violazioni di tutto il codice di produzione sotto `src`.
fn violations_under(src: &Path, known: &[Known]) -> Vec<Violation> {
    let mut files = Vec::new();
    rust_files(src, &mut files);
    let scans: Vec<_> = files
        .iter()
        .map(|file| {
            let source = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            (file, scan(&source))
        })
        .collect();
    let test_files: HashSet<PathBuf> =
        scans.iter().flat_map(|(file, scan)| test_module_files(file, &scan.test_modules)).collect();
    let test_dirs: Vec<PathBuf> = test_files.iter().map(|file| test_module_dir(file)).collect();
    scans
        .iter()
        .filter(|(file, _)| !test_files.contains(*file) && !test_dirs.iter().any(|dir| file.starts_with(dir)))
        .flat_map(|(file, scan)| violations_in(&relative(src, file), &scan.tokens, known))
        .collect()
}

fn report(violations: &[Violation]) -> String {
    let lines: Vec<_> = violations.iter().map(|v| format!("  src/{}:{}: {}", v.file, v.line, v.what)).collect();
    format!(
        "Il core nomina un connettore specifico o un connettore importa troppo (il core offre dei contratti; un \
         connettore usa il contesto).\n{}\nSposta il dettaglio nella cartella del connettore, dietro i contratti. \
         Guida: docs/CONNECTORS.MD",
        lines.join("\n")
    )
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn the_core_does_not_name_a_specific_connector() {
    let found = violations_under(&src_dir(), KNOWN);
    assert!(found.is_empty(), "{}", report(&found));
}

#[test]
fn every_connector_folder_is_known_and_registered() {
    let mut folders: Vec<String> = std::fs::read_dir(src_dir().join("connectors"))
        .expect("cartella connectors")
        .map(|e| e.expect("voce di cartella").path())
        .filter(|p| p.is_dir())
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
        .filter(|name| is_connector_folder(name))
        .collect();
    folders.sort();
    let mut known: Vec<_> = KNOWN.iter().map(|c| c.folder.to_string()).collect();
    known.sort();
    assert_eq!(folders, known, "le cartelle di connectors/ e KNOWN (architecture.rs) devono coincidere");
    for connector in KNOWN {
        assert!(registry::find(connector.id).is_some(), "{} non e' nel registro", connector.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC: &[Known] = &[Known { folder: "telegram", id: "telegram" }];

    fn violations_of(path: &str, source: &str) -> Vec<Violation> {
        violations_in(path, &scan(source).tokens, SYNTHETIC)
    }

    fn what(violations: &[Violation]) -> Vec<&str> {
        violations.iter().map(|v| v.what.as_str()).collect()
    }

    #[test]
    fn the_known_list_matches_the_production_registry() {
        let registered: Vec<_> = registry::all().iter().map(|c| c.descriptor().id).collect();
        assert_eq!(registered, KNOWN.iter().map(|c| c.id).collect::<Vec<_>>());
    }

    #[test]
    fn module_and_id_are_reported_outside_the_connector_folder() {
        let source = "use crate::connectors::telegram;\nfn f() { let a = \"telegram\"; let b = \"x-telegram-y\"; }";
        let found = violations_of("workspaces/store.rs", source);
        assert_eq!(found.iter().map(|v| v.line).collect::<Vec<_>>(), vec![1, 2, 2], "{found:?}");
        assert!(what(&found)[0].contains("il modulo") && what(&found)[1].contains("telegram"));
        // Anche dentro `connectors/` ma fuori dalla cartella del connettore.
        assert!(!violations_of("connectors/model.rs", source).is_empty());
        assert!(!violations_of("connectors/commands.rs", "const P: &str = \"telegram\";").is_empty());
    }

    #[test]
    fn the_connector_folder_may_name_itself() {
        let source = "const ID: &str = \"telegram\"; mod telegram {}";
        assert!(violations_of("connectors/telegram/mod.rs", source).is_empty());
        assert!(violations_of("connectors/telegram/api.rs", source).is_empty());
        // Il nome di una cartella non e' un prefisso.
        assert!(!violations_of("connectors/telegram_x/mod.rs", source).is_empty());
    }

    #[test]
    fn only_registration_files_may_name_the_module() {
        let module = "mod telegram; static C: &[u8] = &[telegram::Telegram];";
        assert!(violations_of("connectors/registry.rs", module).is_empty());
        assert!(violations_of("connectors.rs", "mod telegram;").is_empty());
        assert!(!violations_of("connectors/files.rs", module).is_empty());
        // Le stringhe restano vietate anche nel registro.
        assert!(!violations_of("connectors/registry.rs", "fn f() { find(\"telegram\"); }").is_empty());
    }

    #[test]
    fn comments_tests_and_other_names_are_not_reported() {
        let source = "\
            // usa telegram e \"telegram\"\n\
            /// il modulo telegram\n\
            fn f() { let _ = \"telegrafo\"; let _ = \"tele gram\"; }\n\
            #[cfg(test)]\n\
            mod tests { fn t() { let _ = \"telegram\"; } }\n";
        assert!(violations_of("board.rs", source).is_empty());
    }

    #[test]
    fn a_connector_does_not_use_reqwest_directly() {
        let found = violations_of("connectors/telegram/api.rs", "use reqwest::Client; fn f() { reqwest::get(\"x\"); }");
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(what(&found)[0].contains("ctx.http()"));
        // Il client del contesto e' nel core.
        assert!(violations_of("connectors/runtime/http.rs", "use reqwest::Client;").is_empty());
        assert!(violations_of("connectors/contracts/context.rs", "use reqwest::Client;").is_empty());
    }

    #[test]
    fn a_connector_imports_from_hub_only_the_allowed_items() {
        let ok = "use crate::hub::ingress::ExternalDelivery;\nuse crate::hub::{Author, MessageOrigin};\nuse crate::hub::attachments::IncomingFile;";
        assert!(violations_of("connectors/telegram/mod.rs", ok).is_empty());
        let found = violations_of("connectors/telegram/mod.rs", "use crate::hub::send::send_user_message;");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(what(&found)[0].contains("crate::hub::send"));
        let found = violations_of("connectors/telegram/mod.rs", "fn f() { crate::hub::forget_workspace(a, b); }");
        assert_eq!(found.len(), 1, "{found:?}");
        let found =
            violations_of("connectors/telegram/mod.rs", "use crate::hub::{Author, settings::HubSettings};\nfn f() {}");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(what(&found)[0].contains("hub::settings"));
        // Il core dei connettori (non una cartella di connettore) puo' usare la porta verso l'HUB.
        assert!(violations_of("connectors/contracts/context.rs", "use crate::hub::send::UserMessage;").is_empty());
        assert!(violations_of("connectors/runtime/hub_port.rs", "use crate::hub::send::UserMessage;").is_empty());
    }

    #[test]
    fn the_group_import_window_stops_at_the_next_item() {
        let source = "use crate::hub::{Author, Message};\nuse crate::connectors::state::State;\nfn f(state: u8) {}";
        assert!(violations_of("connectors/telegram/mod.rs", source).is_empty());
    }

    #[test]
    fn connector_folders_are_the_non_core_ones() {
        assert_eq!(connector_folder_of("connectors/telegram/api.rs"), Some("telegram"));
        assert_eq!(connector_folder_of("connectors/registry.rs"), None);
        assert_eq!(connector_folder_of("hub/send.rs"), None);
        assert!(is_connector_folder("telegram") && !is_connector_folder("runtime") && !is_connector_folder("model"));
    }

    #[test]
    fn test_module_files_follow_the_module_layout() {
        let files = |file: &str| test_module_files(Path::new(file), &["tests".to_string()]);
        assert_eq!(
            files("src/connectors/files.rs"),
            vec![PathBuf::from("src/connectors/files/tests.rs"), "src/connectors/files/tests/mod.rs".into()]
        );
        assert_eq!(
            test_module_dir(Path::new("src/connectors/files/tests.rs")),
            PathBuf::from("src/connectors/files/tests")
        );
    }

    #[test]
    fn the_report_points_to_the_design_doc() {
        let message = report(&[Violation { file: "a.rs".into(), line: 3, what: "il modulo `x`".into() }]);
        assert!(message.contains("src/a.rs:3") && message.contains("docs/CONNECTORS.MD"), "{message}");
    }
}
