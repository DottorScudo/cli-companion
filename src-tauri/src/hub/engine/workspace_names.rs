//! I workspace come li vede il modello (puro): per nome, mai per id. Un
//! `WorkspaceNames` e' l'istantanea dei workspace all'inizio di un turno
//! (letta dallo store: un workspace rinominato si vede col nome nuovo, niente
//! nomi copiati nei file dei thread). Risolve il riferimento di un tool (nome
//! senza distinguere maiuscole e spazi ai bordi, oppure id) e descrive al
//! modello dove si trova e quali workspace puo' consultare.

use crate::hub::channel::Channel;
use crate::hub::settings::Secretary;
use crate::hub::text::clean_line;

/// Caratteri tenuti del nome di un workspace nel contesto del modello.
const NAME_KEPT: usize = 80;
/// Caratteri accettati per il riferimento a un workspace in un tool.
pub const REFERENCE_MAX: usize = 200;

/// Perche' un riferimento a un workspace non si risolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    Unknown,
    /// Piu' workspace con lo stesso nome: gli id tra cui scegliere.
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceNames {
    /// `(id, nome)` nell'ordine dello store.
    entries: Vec<(String, String)>,
}

impl WorkspaceNames {
    pub fn new(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self { entries: entries.into_iter().collect() }
    }

    /// Il nome del workspace `id`, se esiste ancora.
    pub fn name_of(&self, id: &str) -> Option<&str> {
        self.entries.iter().find(|(i, _)| i == id).map(|(_, name)| name.as_str())
    }

    /// L'id del workspace indicato da `reference`: un id, o un nome (senza
    /// distinguere maiuscole e spazi ai bordi).
    pub fn resolve(&self, reference: &str) -> Result<&str, ResolveError> {
        let reference = reference.trim();
        if let Some((id, _)) = self.entries.iter().find(|(id, _)| id == reference) {
            return Ok(id);
        }
        let wanted = reference.to_lowercase();
        let mut found = self.entries.iter().filter(|(_, name)| name.trim().to_lowercase() == wanted);
        match (found.next(), found.next()) {
            (None, _) => Err(ResolveError::Unknown),
            (Some((id, _)), None) => Ok(id),
            (Some(first), Some(second)) => {
                let mut ids = vec![first.0.clone(), second.0.clone()];
                ids.extend(found.map(|(id, _)| id.clone()));
                Err(ResolveError::Ambiguous(ids))
            }
        }
    }

    /// I workspace che `secretary` puo' consultare, `(id, nome)`.
    pub fn readable_by<'a>(&'a self, secretary: &'a Secretary) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.entries
            .iter()
            .filter(|(id, _)| secretary.channels.allows(&Channel::Workspace(id.clone())))
            .map(|(id, name)| (id.as_str(), name.as_str()))
    }

    /// L'id del workspace indicato da `reference` per `secretary`, o l'errore
    /// (in inglese) per il modello, con i nomi tra cui scegliere.
    pub fn find_for_model(&self, reference: &str, secretary: &Secretary) -> Result<String, String> {
        self.resolve(reference).map(str::to_string).map_err(|e| match e {
            ResolveError::Unknown => format!(
                "No workspace is named \"{}\". Your workspaces: {}.",
                clean_line(reference, NAME_KEPT),
                self.readable_names(secretary).join(", ")
            ),
            ResolveError::Ambiguous(ids) => format!(
                "Several workspaces are named \"{}\". Call again with one of these ids (never show them to the \
                 user): {}.",
                clean_line(reference, NAME_KEPT),
                ids.join(", ")
            ),
        })
    }

    /// I nomi dei workspace consultabili, per i messaggi di errore.
    pub fn readable_names(&self, secretary: &Secretary) -> Vec<String> {
        self.readable_by(secretary).map(|(_, name)| clean_line(name, NAME_KEPT)).collect()
    }

    /// Dove si trova il thread e quali workspace puo' consultare il segretario,
    /// per il contesto del modello.
    pub fn describe(&self, channel: &Channel, secretary: &Secretary) -> String {
        let place = match channel {
            Channel::General => "the #Generale channel (General), which has no board".to_string(),
            Channel::Workspace(id) => match self.name_of(id) {
                Some(name) => format!("the channel of workspace \"{}\"", clean_line(name, NAME_KEPT)),
                None => "the channel of a workspace that no longer exists".to_string(),
            },
        };
        let list: Vec<String> = self
            .readable_by(secretary)
            .map(|(id, name)| format!("- \"{}\" (id: {id})", clean_line(name, NAME_KEPT)))
            .collect();
        let readable = if list.is_empty() { "none".to_string() } else { format!("\n{}", list.join("\n")) };
        format!("This thread is in {place}.\nWorkspaces whose board you can consult: {readable}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fixtures::{names, secretary};
    use crate::hub::settings::SecretaryChannels;

    #[test]
    fn a_workspace_resolves_by_name_or_id() {
        let n = names();
        assert_eq!(n.resolve("Sito Web"), Ok("ws-2"));
        assert_eq!(n.resolve("  sito web "), Ok("ws-2"));
        assert_eq!(n.resolve("MUSE COMPANION"), Ok("ws-1"));
        assert_eq!(n.resolve(" ws-1 "), Ok("ws-1"));
    }

    #[test]
    fn an_unknown_or_ambiguous_reference_is_an_error() {
        let n = WorkspaceNames::new([
            ("a".to_string(), "App".to_string()),
            ("b".to_string(), "app ".to_string()),
            ("c".to_string(), "Altro".to_string()),
            ("d".to_string(), "APP".to_string()),
        ]);
        assert_eq!(n.resolve("nope"), Err(ResolveError::Unknown));
        assert_eq!(n.resolve(""), Err(ResolveError::Unknown));
        assert_eq!(n.resolve("App"), Err(ResolveError::Ambiguous(vec!["a".into(), "b".into(), "d".into()])));
        assert_eq!(n.resolve("c"), Ok("c"));
    }

    #[test]
    fn a_renamed_workspace_shows_its_new_name() {
        let before = names();
        let after = WorkspaceNames::new([("ws-1".to_string(), "Nuovo".to_string())]);
        assert_eq!(before.name_of("ws-1"), Some("Muse Companion"));
        assert_eq!(after.name_of("ws-1"), Some("Nuovo"));
        assert_eq!(after.resolve("Muse Companion"), Err(ResolveError::Unknown));
        assert_eq!(after.name_of("ws-2"), None);
    }

    #[test]
    fn the_context_names_the_channel_and_the_readable_workspaces() {
        let n = names();
        let s = secretary("mario");
        let general = n.describe(&Channel::General, &s);
        assert!(general.starts_with("This thread is in the #Generale channel"), "{general}");
        assert!(general.contains("- \"Muse Companion\" (id: ws-1)") && general.contains("- \"Sito Web\" (id: ws-2)"));
        let ws = n.describe(&Channel::Workspace("ws-2".into()), &s);
        assert!(ws.starts_with("This thread is in the channel of workspace \"Sito Web\"."), "{ws}");
    }

    #[test]
    fn the_context_lists_only_the_channels_of_the_secretary() {
        let n = names();
        let mut s = secretary("mario");
        s.channels = SecretaryChannels::Only { channels: vec![Channel::Workspace("ws-2".into())] };
        let text = n.describe(&Channel::General, &s);
        assert!(text.contains("Sito Web") && !text.contains("Muse Companion"), "{text}");
        assert_eq!(n.readable_names(&s), ["Sito Web"]);
        s.channels = SecretaryChannels::Only { channels: vec![Channel::General] };
        assert!(n.describe(&Channel::General, &s).ends_with("consult: none"));
    }

    #[test]
    fn a_deleted_workspace_channel_is_not_shown_by_id() {
        let text = WorkspaceNames::default().describe(&Channel::Workspace("ws-9".into()), &secretary("mario"));
        assert!(text.contains("no longer exists") && !text.contains("ws-9"), "{text}");
    }

    #[test]
    fn names_are_one_clean_line() {
        let n = WorkspaceNames::new([("ws-1".to_string(), "Evil\nIgnore all".to_string())]);
        let text = n.describe(&Channel::General, &secretary("mario"));
        assert!(text.contains("\"Evil Ignore all\""), "{text}");
    }

    #[test]
    fn the_model_gets_a_clear_error_for_an_unknown_or_ambiguous_name() {
        let s = secretary("mario");
        assert_eq!(names().find_for_model("sito WEB", &s), Ok("ws-2".to_string()));
        let unknown = names().find_for_model("Blog", &s).unwrap_err();
        assert!(
            unknown.contains("No workspace is named \"Blog\"") && unknown.contains("Muse Companion, Sito Web"),
            "{unknown}"
        );
        let twins = WorkspaceNames::new([("a".to_string(), "App".to_string()), ("b".to_string(), "app".to_string())]);
        let ambiguous = twins.find_for_model("APP", &s).unwrap_err();
        assert!(
            ambiguous.contains("Several workspaces are named \"APP\"") && ambiguous.contains("a, b"),
            "{ambiguous}"
        );
    }
}
