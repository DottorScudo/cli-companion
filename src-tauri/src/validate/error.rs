//! Errore di validazione tipizzato: un codice stabile, i parametri e l'eventuale
//! contesto (avvio, repository, lista di regole). Il testo non e' nel chiamante:
//! lo produce questo modulo, in italiano per l'interfaccia e in inglese per gli
//! strumenti MCP (letti dal modello). Ogni codice porta entrambe le lingue nella
//! stessa riga della tabella, quindi non puo' averne una sola.

use std::fmt;

/// Segnaposto dei modelli di messaggio.
const PLACEHOLDER_MAX: &str = "{max}";
const PLACEHOLDER_MIN: &str = "{min}";
const PLACEHOLDER_VALUE: &str = "{value}";

/// Definisce `ErrorCode`, l'elenco `ALL` e i due modelli (italiano, inglese) di
/// ciascun codice. Un codice senza i due modelli non compila.
macro_rules! error_codes {
    ($($variant:ident => ($it:literal, $en:literal),)+) => {
        /// Motivo (stabile) per cui un valore e' stato rifiutato.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ErrorCode {
            $($variant,)+
        }

        impl ErrorCode {
            /// Tutti i codici (per i test di completezza dei messaggi).
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$variant,)+];

            /// Modelli `(italiano, inglese)`; i segnaposto sono `{max}`, `{min}`, `{value}`.
            fn templates(self) -> (&'static str, &'static str) {
                match self {
                    $(ErrorCode::$variant => ($it, $en),)+
                }
            }
        }
    };
}

error_codes! {
    // Impostazioni dei worker.
    RulesTooLong => ("regole troppo lunghe (max {max} caratteri)", "worker_rules too long (max {max} characters)"),
    RulesControlChars => (
        "le regole non possono contenere caratteri di controllo",
        "worker_rules cannot contain control characters"
    ),
    PermissionInvalid => (
        "permesso non valido (usa plan, default, accept-edits o bypass)",
        "invalid worker_permission (use plan, default, accept-edits or bypass)"
    ),
    ModelInvalid => (
        "modello non valido (solo lettere, cifre, '.', '_' e '-', max {max})",
        "invalid worker_model (letters, digits, '.', '_' and '-' only, max {max} characters)"
    ),
    PermissionRulesTooMany => ("troppe regole (massimo {max})", "too many rules (max {max})"),
    PermissionRuleTooLong => (
        "regola \"{value}\": piu' lunga di {max} caratteri",
        "rule \"{value}\": longer than {max} characters"
    ),
    PermissionRuleControlChars => (
        "regola \"{value}\": contiene a capo o caratteri di controllo",
        "rule \"{value}\": contains newlines or control characters"
    ),
    PermissionRuleBadShape => (
        "regola \"{value}\": forma non valida per l'agente dei worker",
        "rule \"{value}\": not a valid rule for the worker agent"
    ),
    PermissionRulesUnsupported => (
        "l'agente dei worker non supporta le regole di permesso",
        "the worker agent does not support permission rules"
    ),
    PermissionRuleTakesNoArguments => (
        "regola \"{value}\": questo tipo di regola non accetta argomenti",
        "rule \"{value}\": this kind of rule takes no arguments"
    ),
    // Configurazione di un repository.
    BaseBranchInvalid => ("branch base non valido: {value}", "invalid base_branch: {value}"),
    ContainerRootInvalid => ("percorso nel container non valido", "invalid container_root"),
    CopyPathNotRelative => (
        "il percorso da copiare deve essere relativo al repository: {value}",
        "copy_ignored paths must be relative to the repository: {value}"
    ),
    CopyPathEscapes => (
        "percorso da copiare non valido (niente '..' ne' la radice): {value}",
        "invalid copy_ignored path (no '..', not the repository root): {value}"
    ),
    CopyPathInvalid => (
        "percorso da copiare non valido: {value}",
        "invalid copy_ignored path (no control characters, max {max} characters): {value}"
    ),
    CopyPathsTooMany => ("troppi percorsi da copiare (max {max})", "too many copy_ignored paths (max {max})"),
    SetupCommandMultiline => (
        "il comando di setup deve stare su una riga: {value}",
        "setup commands must be single-line: {value}"
    ),
    CleanupCommandMultiline => (
        "il comando di pulizia deve stare su una riga: {value}",
        "cleanup commands must be single-line: {value}"
    ),
    SetupCommandTooLong => (
        "comando di setup troppo lungo (max {max} caratteri)",
        "setup command too long (max {max} characters)"
    ),
    CleanupCommandTooLong => (
        "comando di pulizia troppo lungo (max {max} caratteri)",
        "cleanup command too long (max {max} characters)"
    ),
    SetupCommandsTooMany => ("troppi comandi di setup (max {max})", "too many setup commands (max {max})"),
    CleanupCommandsTooMany => ("troppi comandi di pulizia (max {max})", "too many cleanup commands (max {max})"),
    RepoInvalid => ("repository non valido: {value}", "{value}"),
    // Test E2E di un repository.
    E2eCommandMissing => ("comando E2E mancante", "e2e command is missing"),
    E2eCommandMultiline => ("il comando E2E deve stare su una riga", "e2e command must be single-line"),
    E2eCommandTooLong => (
        "comando E2E troppo lungo (max {max} caratteri)",
        "e2e command too long (max {max} characters)"
    ),
    E2eTimeoutInvalid => (
        "timeout E2E non valido (da {min} a {max} minuti)",
        "invalid e2e timeout_min (from {min} to {max} minutes)"
    ),
    E2eJunitMissing => ("percorso JUnit E2E mancante", "e2e junit path is missing"),
    E2eJunitNotRelative => (
        "il percorso JUnit E2E deve essere relativo al repository",
        "e2e junit path must be relative to the repository"
    ),
    E2eJunitEscapes => ("percorso JUnit E2E non valido (niente '..')", "invalid e2e junit path (no '..')"),
    E2eJunitInvalid => (
        "percorso JUnit E2E non valido (max {max} caratteri)",
        "invalid e2e junit path (no control characters, max {max} characters)"
    ),
    // Avvii personalizzati.
    LaunchNameMissing => ("nome avvio mancante", "launch name is missing"),
    LaunchNameTooLong => ("nome avvio troppo lungo (max {max} caratteri)", "launch name too long (max {max} characters)"),
    LaunchCommandMissing => ("comando avvio mancante", "launch command is missing"),
    LaunchNotFound => ("avvio non trovato", "launch not found"),
    LaunchIdUnknown => (
        "avvio non trovato: {value}",
        "launch not found: an id must be one of the workspace's current launches: {value}"
    ),
    DefaultLaunchInvalid => (
        "avvio predefinito non valido",
        "invalid default_launch (use \"shell\", the id of a workspace launch or a builtin launch id)"
    ),
    // Proposte di impostazioni.
    DeliveryEmpty => ("delivery: nessun campo da modificare", "delivery: has no fields"),
    LaunchesTooMany => (
        "troppi avvii nella proposta (max {max})",
        "too many launches in the proposal (max {max})"
    ),
    LaunchIdDuplicate => ("id avvio duplicato: {value}", "duplicate launch id: {value}"),
    ReposTooMany => ("troppi repository nella proposta (max {max})", "too many repos in the proposal (max {max})"),
    RepoKeyDuplicate => ("repository duplicato nella proposta", "duplicate repo key"),
    RepoPatchEmpty => ("nessun campo da modificare", "has no fields"),
}

/// Lista di regole di permesso a cui appartiene un errore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleList {
    Allow,
    Deny,
}

/// Dove, nella proposta, e' stato trovato l'errore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Voce `launches[i]`.
    Launch(usize),
    /// Voce `repos['chiave']`.
    Repo(String),
    /// Lista `worker_allow` / `worker_deny`.
    Rules(RuleList),
}

impl Scope {
    fn prefix(&self, english: bool) -> String {
        match self {
            Scope::Launch(index) => format!("launches[{index}]: "),
            Scope::Repo(key) => format!("repos['{key}']: "),
            Scope::Rules(RuleList::Allow) => if english { "worker_allow: " } else { "Consentiti: " }.to_string(),
            Scope::Rules(RuleList::Deny) => if english { "worker_deny: " } else { "Vietati: " }.to_string(),
        }
    }
}

/// Valore rifiutato: codice, parametri del messaggio e contesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    code: ErrorCode,
    scope: Option<Scope>,
    max: Option<usize>,
    min: Option<usize>,
    value: Option<String>,
}

impl ValidationError {
    pub fn new(code: ErrorCode) -> Self {
        Self { code, scope: None, max: None, min: None, value: None }
    }

    /// Limite superiore citato nel messaggio (`{max}`).
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    /// Limite inferiore citato nel messaggio (`{min}`).
    pub fn min(mut self, min: usize) -> Self {
        self.min = Some(min);
        self
    }

    /// Valore rifiutato (o dettaglio) citato nel messaggio (`{value}`).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Colloca l'errore in una voce di `launches`.
    pub fn in_launch(self, index: usize) -> Self {
        self.in_scope(Scope::Launch(index))
    }

    /// Colloca l'errore in una voce di `repos`.
    pub fn in_repo(self, key: &str) -> Self {
        self.in_scope(Scope::Repo(key.to_string()))
    }

    /// Colloca l'errore in una delle liste di regole di permesso.
    pub fn in_rules(self, list: RuleList) -> Self {
        self.in_scope(Scope::Rules(list))
    }

    fn in_scope(mut self, scope: Scope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Messaggio per l'interfaccia.
    pub fn message_it(&self) -> String {
        self.render(false)
    }

    /// Messaggio per gli strumenti MCP (letto dal modello).
    pub fn message_en(&self) -> String {
        self.render(true)
    }

    fn render(&self, english: bool) -> String {
        let (it, en) = self.code.templates();
        let mut text = if english { en } else { it }.to_string();
        for (placeholder, param) in
            [(PLACEHOLDER_MAX, self.max.map(|n| n.to_string())), (PLACEHOLDER_MIN, self.min.map(|n| n.to_string()))]
        {
            if let Some(param) = param {
                text = text.replace(placeholder, &param);
            }
        }
        if let Some(value) = &self.value {
            text = text.replace(PLACEHOLDER_VALUE, value);
        }
        match &self.scope {
            Some(scope) => format!("{}{text}", scope.prefix(english)),
            None => text,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message_it())
    }
}

impl std::error::Error for ValidationError {}

/// I comandi dell'interfaccia riportano gli errori come testo italiano.
impl From<ValidationError> for String {
    fn from(e: ValidationError) -> String {
        e.message_it()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(code: ErrorCode) -> ValidationError {
        ValidationError::new(code).max(7).min(2).value("val")
    }

    #[test]
    fn every_code_has_both_messages_with_all_parameters_filled() {
        for &code in ErrorCode::ALL {
            let e = sample(code);
            let (it, en) = (e.message_it(), e.message_en());
            for text in [&it, &en] {
                assert!(!text.is_empty(), "{code:?}");
                assert!(!text.contains('{') && !text.contains('}'), "{code:?}: segnaposto non sostituito in {text:?}");
            }
            assert_ne!(it, en, "{code:?}: italiano e inglese coincidono");
        }
    }

    #[test]
    fn the_code_list_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for &code in ErrorCode::ALL {
            assert!(seen.insert(code), "{code:?} duplicato");
        }
    }

    #[test]
    fn parameters_appear_in_the_messages() {
        let e = ValidationError::new(ErrorCode::E2eTimeoutInvalid).min(1).max(60);
        assert_eq!(e.message_it(), "timeout E2E non valido (da 1 a 60 minuti)");
        assert_eq!(e.message_en(), "invalid e2e timeout_min (from 1 to 60 minutes)");
        let e = ValidationError::new(ErrorCode::BaseBranchInvalid).value("a b");
        assert_eq!(e.message_it(), "branch base non valido: a b");
        assert_eq!(e.message_en(), "invalid base_branch: a b");
    }

    #[test]
    fn scope_prefixes_both_languages() {
        let e = ValidationError::new(ErrorCode::LaunchNameMissing).in_launch(2);
        assert_eq!(
            (e.message_it().as_str(), e.message_en().as_str()),
            ("launches[2]: nome avvio mancante", "launches[2]: launch name is missing")
        );
        let e = ValidationError::new(ErrorCode::ContainerRootInvalid).in_repo("api");
        assert_eq!(e.message_en(), "repos['api']: invalid container_root");
        let e = ValidationError::new(ErrorCode::PermissionRulesTooMany).max(100).in_rules(RuleList::Deny);
        assert_eq!(e.message_it(), "Vietati: troppe regole (massimo 100)");
        assert_eq!(e.message_en(), "worker_deny: too many rules (max 100)");
    }

    #[test]
    fn converts_to_the_italian_text_for_the_interface() {
        let e = ValidationError::new(ErrorCode::LaunchNotFound);
        assert_eq!(e.to_string(), "avvio non trovato");
        assert_eq!(String::from(e), "avvio non trovato");
    }

    #[test]
    fn a_value_with_placeholders_is_not_expanded_twice() {
        let e = ValidationError::new(ErrorCode::BaseBranchInvalid).value("{max}");
        assert_eq!(e.message_en(), "invalid base_branch: {max}");
    }
}
