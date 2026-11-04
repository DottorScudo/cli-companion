//! Contratto di automazione: avviare una sessione senza utente (worker).

use crate::platform::TargetKind;
use ts_rs::TS;

/// Livello di permessi con cui parte una sessione automatica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TS)]
// Non ha derive serde: il testo sul filo e' `as_str`, che e' il kebab-case dei nomi.
#[ts(export, rename_all = "kebab-case")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
    Bypass,
}

impl PermissionMode {
    /// Tutti i livelli, dal piu' restrittivo.
    pub const ALL: [PermissionMode; 4] =
        [PermissionMode::Plan, PermissionMode::Default, PermissionMode::AcceptEdits, PermissionMode::Bypass];

    /// Valore ammesso dall'RPC `spawn_session`: "default" | "accept-edits" |
    /// "plan" | "bypass".
    pub fn parse(s: &str) -> Option<PermissionMode> {
        match s {
            "default" => Some(PermissionMode::Default),
            "accept-edits" => Some(PermissionMode::AcceptEdits),
            "plan" => Some(PermissionMode::Plan),
            "bypass" => Some(PermissionMode::Bypass),
            _ => None,
        }
    }

    /// Inverso di `parse`: stesso vocabolario di RPC, impostazioni e UI.
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionMode::Plan => "plan",
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "accept-edits",
            PermissionMode::Bypass => "bypass",
        }
    }

    /// Permissivita' crescente: plan < default < accept-edits < bypass.
    pub fn rank(self) -> u8 {
        match self {
            PermissionMode::Plan => 0,
            PermissionMode::Default => 1,
            PermissionMode::AcceptEdits => 2,
            PermissionMode::Bypass => 3,
        }
    }
}

/// Richiesta di avvio di una sessione automatica.
pub struct AutomationRequest<'a> {
    /// ASCII sicuro (`[A-Za-z0-9 .,:;_()'-]`, garantito dal chiamante):
    /// finisce in una riga di comando digitata nella shell del tab.
    pub bootstrap_prompt: &'a str,
    pub permission_mode: PermissionMode,
    /// Gia' validato: forma generica sicura (`validate::valid_model_shape`) e regola
    /// dei modelli dell'agente (`ModelCatalog::is_valid`). L'integrazione rivalida.
    pub model: Option<&'a str>,
    /// Regole di permesso dei worker preparate dal contratto `WorkerPermissions`
    /// della stessa integrazione, gia' nel formato di path del target.
    pub permissions_file: Option<&'a str>,
}

/// Come si avvia una sessione automatica: un comando e, se l'agente si
/// configura via ambiente, le variabili da impostare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationLaunch {
    /// Riga digitata nella shell del tab, come sempre.
    pub command: String,
    /// Variabili impostate dal core sul processo del tab PRIMA che la shell
    /// parta: arrivano all'agente in PowerShell, cmd e bash (niente sintassi
    /// di shell). Su WSL passano con `WSLENV`. I valori sono letterali, mai
    /// interpretati da una shell; vanno gia' nel formato di path del target.
    /// Nomi e valori devono superare `is_valid_env_name` / `is_valid_env_value`.
    pub env: Vec<(String, String)>,
}

/// Avvio di sessioni automatiche.
pub trait WorkerAutomation: Sync {
    /// Come avviare una sessione automatica. `command` va digitato nella shell
    /// del tab: `env_kind` e' la famiglia del target e la stringa deve
    /// funzionare in PowerShell, cmd e bash/zsh. `None` = richiesta non
    /// rappresentabile in modo sicuro (caratteri speciali nel prompt, nel
    /// modello o nel path).
    fn command(&self, env_kind: TargetKind, req: &AutomationRequest) -> Option<AutomationLaunch>;
}

/// Nomi che il core si riserva: l'integrazione non puo' impostarli.
pub const RESERVED_ENV_NAMES: [&str; 4] = ["WSLENV", "PATH", "TERM", "COLORTERM"];
/// Prefisso riservato alle variabili del core (sessione, token, porta).
pub const RESERVED_ENV_PREFIX: &str = "CLI_COMPANION_";
/// Lunghezza massima di un valore di variabile, in byte.
pub const MAX_ENV_VALUE_LEN: usize = 32 * 1024;

/// Nome ammesso per una variabile d'ambiente di un'integrazione:
/// `^[A-Z_][A-Z0-9_]{0,63}$`, non riservato.
pub fn is_valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let shape = chars.next().is_some_and(|c| c.is_ascii_uppercase() || c == '_')
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    shape && !name.starts_with(RESERVED_ENV_PREFIX) && !RESERVED_ENV_NAMES.contains(&name)
}

/// Valore ammesso: al piu' 32 KiB, senza caratteri di controllo (NUL, a capo).
pub fn is_valid_env_value(value: &str) -> bool {
    value.len() <= MAX_ENV_VALUE_LEN && !value.chars().any(|c| c.is_control())
}

/// Primo nome non valido (nome, valore o duplicato) fra le `env` di un avvio.
pub fn first_invalid_env_name(env: &[(String, String)]) -> Option<&str> {
    env.iter().enumerate().find_map(|(i, (name, value))| {
        let duplicate = env[..i].iter().any(|(earlier, _)| earlier == name);
        (duplicate || !is_valid_env_name(name) || !is_valid_env_value(value)).then_some(name.as_str())
    })
}

/// `true` se il path sta in modo letterale tra doppi apici in PowerShell,
/// cmd e bash: l'unica forma ammessa per `AutomationRequest::permissions_file`
/// in una riga di comando.
pub fn is_shell_safe_path(p: &str) -> bool {
    !p.is_empty() && !p.chars().any(|c| matches!(c, '"' | '$' | '`' | '%' | '!') || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_safe_paths() {
        assert!(is_shell_safe_path(r"C:\Users\Mario Rossi\AppData\x.json"));
        assert!(is_shell_safe_path("/mnt/c/Users/o'neil/x.json"));
        for bad in [r"C:\a$b\x.json", r"C:\a`b", r"C:\100%\x", "C:\\a\"b", r"C:\hi!\x", ""] {
            assert!(!is_shell_safe_path(bad), "{bad:?}");
        }
    }

    #[test]
    fn env_names() {
        for good in ["A", "_", "OPENCODE_CONFIG_CONTENT", "X1_2", &"A".repeat(64)] {
            assert!(is_valid_env_name(good), "{good:?}");
        }
        for bad in ["", "a", "Ab", "1A", "A-B", "A B", "A=B", "\u{c0}", "WSLENV", "PATH", "TERM", "COLORTERM"] {
            assert!(!is_valid_env_name(bad), "{bad:?}");
        }
        assert!(!is_valid_env_name(&"A".repeat(65)));
        for reserved in ["CLI_COMPANION_TOKEN", "CLI_COMPANION_", "CLI_COMPANION_X"] {
            assert!(!is_valid_env_name(reserved), "{reserved:?}");
        }
        // Solo il prefisso esatto e' riservato.
        assert!(is_valid_env_name("CLI_COMPANIONX"));
    }

    #[test]
    fn env_values() {
        assert!(is_valid_env_value(""));
        assert!(is_valid_env_value(r#"{"a":"b c","d":[1,2]} $x %z% !"#));
        assert!(is_valid_env_value(&"x".repeat(MAX_ENV_VALUE_LEN)));
        assert!(!is_valid_env_value(&"x".repeat(MAX_ENV_VALUE_LEN + 1)));
        for bad in ["a\nb", "a\rb", "a\0b", "a\tb"] {
            assert!(!is_valid_env_value(bad), "{bad:?}");
        }
    }

    #[test]
    fn invalid_env_is_found_by_name() {
        let e = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        assert_eq!(first_invalid_env_name(&e(&[("A", "1"), ("B", "2")])), None);
        assert_eq!(first_invalid_env_name(&e(&[])), None);
        assert_eq!(first_invalid_env_name(&e(&[("A", "1"), ("a", "2")])), Some("a"));
        assert_eq!(first_invalid_env_name(&e(&[("A", "1"), ("B", "x\ny")])), Some("B"));
        assert_eq!(first_invalid_env_name(&e(&[("A", "1"), ("A", "2")])), Some("A"));
        assert_eq!(first_invalid_env_name(&e(&[("PATH", "x")])), Some("PATH"));
    }
}
