//! Un segretario: un bot con un nome che l'utente sveglia con `@nome` in un
//! thread. Qui il dato e la validazione dei suoi campi; le operazioni
//! sull'elenco sono in `secretaries`. Lo fa parlare il motore (`hub::engine`):
//! un segretario senza system prompt esiste ma non risponde.

use super::tolerant;
use crate::bots::BotImage;
use crate::hub::channel::Channel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lunghezza del nome (l'handle della menzione), in caratteri.
pub const NAME_MAX: usize = 24;
/// Lunghezza del system prompt, in caratteri.
pub const SYSTEM_PROMPT_MAX: usize = 8000;
/// Canali in un elenco esplicito: ben oltre i workspace di un utente.
pub const CHANNELS_MAX: usize = 200;

/// Un tool del motore che il segretario puo' usare: il limite vero di cosa fa
/// sta qui, non nel prompt. L'enum crescera' (`ask_user`): un
/// tool sconosciuto letto dal file si salta (`tolerant::known_items`) senza
/// perdere il resto. Un tool nuovo non si aggiunge da solo ai segretari
/// esistenti: lo sceglie l'utente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubSecretaryTool")]
pub enum SecretaryTool {
    /// Legge lo stato della board del canale.
    BoardStatus,
    /// Legge, in una chiamata, un riepilogo di ogni workspace che il segretario puo' consultare.
    BoardOverview,
    /// Legge un task per intero: specifica, stato, dipendenze, domanda e report.
    TaskDetails,
    /// Legge i messaggi del thread.
    ThreadRead,
    /// Scrive un messaggio nel thread.
    PostMessage,
    /// Apre un task di pianificazione nel workspace del canale, collegato al thread.
    OpenPlanTask,
    /// Accetta o scarta la proposta a cui l'utente sta rispondendo.
    DecideProposal,
    /// Inoltra al worker la risposta dell'utente all'unica domanda in attesa del thread.
    RelayAnswer,
}

impl SecretaryTool {
    /// Tutti i tool, nell'ordine canonico in cui si salvano.
    pub const ALL: [SecretaryTool; 8] = [
        Self::BoardStatus,
        Self::BoardOverview,
        Self::TaskDetails,
        Self::ThreadRead,
        Self::PostMessage,
        Self::OpenPlanTask,
        Self::DecideProposal,
        Self::RelayAnswer,
    ];

    /// Nome sul filo e su disco.
    pub fn code(self) -> &'static str {
        match self {
            Self::BoardStatus => "board_status",
            Self::BoardOverview => "board_overview",
            Self::TaskDetails => "task_details",
            Self::ThreadRead => "thread_read",
            Self::PostMessage => "post_message",
            Self::OpenPlanTask => "open_plan_task",
            Self::DecideProposal => "decide_proposal",
            Self::RelayAnswer => "relay_answer",
        }
    }

    /// Cosa fa una chiamata riuscita: decide l'avviso di audit e il limite di
    /// modifiche per turno (`hub::engine::audit`).
    pub fn class(self) -> ToolClass {
        match self {
            Self::BoardStatus | Self::BoardOverview | Self::TaskDetails | Self::ThreadRead => ToolClass::Read,
            Self::PostMessage | Self::OpenPlanTask | Self::DecideProposal | Self::RelayAnswer => ToolClass::Write,
        }
    }

    fn parse(raw: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|t| t.code() == raw).ok_or_else(|| "tool non riconosciuto".to_string())
    }
}

/// La classe di un tool, secondo l'effetto di una chiamata riuscita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolClass {
    /// Sola lettura: nessun effetto.
    Read,
    /// Una modifica reversibile del thread o della board.
    Write,
    /// Una modifica irreversibile, o che falsa lo stato della board: chiedera' conferma.
    #[expect(dead_code, reason = "nessun tool distruttivo ancora: arriveranno con la conferma dell'utente")]
    Destructive,
}

/// Dove il segretario puo' essere menzionato.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export, rename = "HubSecretaryChannels")]
pub enum SecretaryChannels {
    /// Il Generale e ogni workspace, anche quelli aggiunti dopo.
    #[default]
    All,
    /// Solo questi canali (`general` o id di workspace), senza ripetizioni.
    Only {
        #[ts(type = "string[]")]
        channels: Vec<Channel>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubSecretary")]
pub struct Secretary {
    /// Handle della menzione: `[a-z0-9-]`, 1..=24, unico. Riservato: non diventa
    /// mai il nome di una sessione dei terminali.
    pub name: String,
    /// Immagine pixel-art, con la forma e la validazione di quelle dei bot.
    #[serde(default)]
    pub image: Option<BotImage>,
    /// Letto dal modello (di solito in inglese); vuoto = non configurato.
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default, deserialize_with = "tolerant::known_items")]
    pub tools: Vec<SecretaryTool>,
    /// Assente = tutti; una forma sconosciuta (versione futura) = nessun canale,
    /// mai piu' permessi di quelli scelti.
    #[serde(default, deserialize_with = "channels_or_none")]
    pub channels: SecretaryChannels,
    /// Id del preset (`presets`) da cui è nato: permette di ripristinarne
    /// prompt e tool. Assente nei segretari creati da zero e nei file scritti
    /// prima; un valore illeggibile vale assente.
    #[serde(default, deserialize_with = "tolerant::or_default")]
    pub preset: Option<String>,
}

/// I canali letti dal file; se non si riconoscono, nessun canale (con un avviso).
fn channels_or_none<'de, D: serde::Deserializer<'de>>(de: D) -> Result<SecretaryChannels, D::Error> {
    tolerant::or_else(de, || SecretaryChannels::Only { channels: Vec::new() })
}

impl SecretaryChannels {
    /// Il segretario puo' essere menzionato nel canale (e leggerne la board).
    pub fn allows(&self, channel: &Channel) -> bool {
        match self {
            Self::All => true,
            Self::Only { channels } => channels.contains(channel),
        }
    }
}

impl Secretary {
    /// Ha un system prompt: il motore lo fa rispondere.
    pub fn is_configured(&self) -> bool {
        !self.system_prompt.is_empty()
    }

    /// Il segretario puo' usare il tool.
    pub fn may_use(&self, tool: SecretaryTool) -> bool {
        self.tools.contains(&tool)
    }
}

/// I campi modificabili di un segretario (tutti tranne l'immagine), validati.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretaryDraft {
    pub name: String,
    pub system_prompt: String,
    pub tools: Vec<SecretaryTool>,
    pub channels: SecretaryChannels,
}

impl SecretaryDraft {
    /// Valida l'input del frontend. `channels` `None` = tutti i canali;
    /// `workspace_exists` dice se l'id di un workspace esiste ancora.
    pub fn new(
        name: &str,
        system_prompt: &str,
        tools: &[String],
        channels: Option<&[String]>,
        workspace_exists: impl Fn(&str) -> bool,
    ) -> Result<Self, String> {
        Ok(Self {
            name: normalize_name(name)?,
            system_prompt: normalize_system_prompt(system_prompt)?,
            tools: parse_tools(tools)?,
            channels: parse_channels(channels, workspace_exists)?,
        })
    }
}

/// Trim e minuscolo; poi 1..=`NAME_MAX` caratteri tra `a-z`, `0-9` e `-`, che
/// inizia con una lettera o una cifra e non finisce con `-`.
pub fn normalize_name(raw: &str) -> Result<String, String> {
    let name = raw.trim().to_ascii_lowercase();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(format!("il nome del segretario deve avere da 1 a {NAME_MAX} caratteri"));
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("il nome del segretario può contenere solo lettere minuscole, cifre e -".to_string());
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("il nome del segretario non può iniziare o finire con -".to_string());
    }
    Ok(name)
}

/// Trim, al massimo `SYSTEM_PROMPT_MAX` caratteri; ammessi a capo e
/// tabulazioni, non gli altri caratteri di controllo. Vuoto e' ammesso.
fn normalize_system_prompt(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.chars().count() > SYSTEM_PROMPT_MAX {
        return Err(format!("il system prompt supera i {SYSTEM_PROMPT_MAX} caratteri"));
    }
    if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("il system prompt contiene caratteri non validi".to_string());
    }
    Ok(text.to_string())
}

/// Tool riconosciuti, senza ripetizioni, nell'ordine canonico.
fn parse_tools(raw: &[String]) -> Result<Vec<SecretaryTool>, String> {
    let chosen = raw.iter().map(|t| SecretaryTool::parse(t)).collect::<Result<Vec<_>, _>>()?;
    Ok(SecretaryTool::ALL.into_iter().filter(|t| chosen.contains(t)).collect())
}

/// `None` = tutti. Altrimenti 1..=`CHANNELS_MAX` canali validi ed esistenti,
/// senza ripetizioni, nell'ordine dato.
fn parse_channels(
    raw: Option<&[String]>,
    workspace_exists: impl Fn(&str) -> bool,
) -> Result<SecretaryChannels, String> {
    let Some(raw) = raw else {
        return Ok(SecretaryChannels::All);
    };
    if raw.is_empty() {
        return Err("scegli almeno un canale, oppure tutti".to_string());
    }
    if raw.len() > CHANNELS_MAX {
        return Err(format!("al massimo {CHANNELS_MAX} canali"));
    }
    let mut channels: Vec<Channel> = Vec::with_capacity(raw.len());
    for item in raw {
        let channel = Channel::parse(item)?;
        if let Channel::Workspace(id) = &channel {
            if !workspace_exists(id) {
                return Err("canale non trovato: il workspace non esiste più".to_string());
            }
        }
        if !channels.contains(&channel) {
            channels.push(channel);
        }
    }
    Ok(SecretaryChannels::Only { channels })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn draft(name: &str, prompt: &str, tools: &[&str], channels: Option<&[&str]>) -> Result<SecretaryDraft, String> {
        let channels = channels.map(strings);
        SecretaryDraft::new(name, prompt, &strings(tools), channels.as_deref(), |id| id.starts_with("ws-"))
    }

    #[test]
    fn a_minimal_secretary_is_valid_but_not_configured() {
        let d = draft("mario", "", &[], None).unwrap();
        assert_eq!(d.channels, SecretaryChannels::All);
        assert!(d.tools.is_empty());
        let s = Secretary {
            name: d.name,
            image: None,
            system_prompt: d.system_prompt,
            tools: d.tools,
            channels: d.channels,
            preset: None,
        };
        assert!(!s.is_configured());
        assert!(Secretary { system_prompt: "Report the status.".into(), ..s }.is_configured());
    }

    #[test]
    fn names_are_trimmed_and_lowercased() {
        assert_eq!(normalize_name("  Mario ").unwrap(), "mario");
        assert_eq!(normalize_name("bot-2").unwrap(), "bot-2");
        assert_eq!(normalize_name("7").unwrap(), "7");
        assert!(normalize_name(&"a".repeat(NAME_MAX)).is_ok());
    }

    #[test]
    fn bad_names_are_rejected() {
        for bad in ["", "   ", "ma rio", "mario!", "@mario", "mario_1", "-mario", "mario-", "màrio", "a\nb"] {
            assert!(normalize_name(bad).is_err(), "{bad:?}");
        }
        assert!(normalize_name(&"a".repeat(NAME_MAX + 1)).is_err());
    }

    #[test]
    fn system_prompt_limits() {
        let ok = draft("m", "  Line one.\n\tLine two.  ", &[], None).unwrap();
        assert_eq!(ok.system_prompt, "Line one.\n\tLine two.");
        assert!(draft("m", &"é".repeat(SYSTEM_PROMPT_MAX), &[], None).is_ok());
        assert!(draft("m", &"a".repeat(SYSTEM_PROMPT_MAX + 1), &[], None).is_err());
        assert!(draft("m", "bad \u{0} char", &[], None).is_err());
    }

    #[test]
    fn the_planning_tools_are_chosen_like_the_others() {
        let d = draft("m", "", &["decide_proposal", "open_plan_task"], None).unwrap();
        assert_eq!(d.tools, vec![SecretaryTool::OpenPlanTask, SecretaryTool::DecideProposal]);
        // Un segretario salvato prima dei nuovi tool non li riceve da solo.
        let s: Secretary = serde_json::from_str(r#"{"name":"mario","tools":["board_status"]}"#).unwrap();
        assert!(!s.may_use(SecretaryTool::OpenPlanTask) && !s.may_use(SecretaryTool::DecideProposal));
    }

    #[test]
    fn relay_answer_is_chosen_and_never_added_to_existing_secretaries() {
        let d = draft("m", "", &["relay_answer", "thread_read"], None).unwrap();
        assert_eq!(d.tools, vec![SecretaryTool::ThreadRead, SecretaryTool::RelayAnswer]);
        let s: Secretary =
            serde_json::from_str(r#"{"name":"mario","tools":["board_status","decide_proposal"]}"#).unwrap();
        assert!(!s.may_use(SecretaryTool::RelayAnswer));
        let saved: Secretary = serde_json::from_str(r#"{"name":"mario","tools":["relay_answer"]}"#).unwrap();
        assert_eq!(saved.tools, vec![SecretaryTool::RelayAnswer]);
    }

    #[test]
    fn tools_are_deduplicated_in_canonical_order() {
        let d = draft("m", "", &["post_message", "board_status", "post_message"], None).unwrap();
        assert_eq!(d.tools, vec![SecretaryTool::BoardStatus, SecretaryTool::PostMessage]);
        for t in SecretaryTool::ALL {
            assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{}\"", t.code()));
        }
    }

    #[test]
    fn every_tool_has_its_class() {
        let class = |t: SecretaryTool| (t, t.class());
        assert_eq!(
            SecretaryTool::ALL.map(class),
            [
                (SecretaryTool::BoardStatus, ToolClass::Read),
                (SecretaryTool::BoardOverview, ToolClass::Read),
                (SecretaryTool::TaskDetails, ToolClass::Read),
                (SecretaryTool::ThreadRead, ToolClass::Read),
                (SecretaryTool::PostMessage, ToolClass::Write),
                (SecretaryTool::OpenPlanTask, ToolClass::Write),
                (SecretaryTool::DecideProposal, ToolClass::Write),
                (SecretaryTool::RelayAnswer, ToolClass::Write),
            ]
        );
    }

    #[test]
    fn unknown_tools_are_rejected_from_the_frontend() {
        assert!(draft("m", "", &["ask_user"], None).is_err());
        assert!(draft("m", "", &["Board_Status"], None).is_err());
    }

    #[test]
    fn channels_are_validated_and_deduplicated() {
        let d = draft("m", "", &[], Some(&["general", "ws-1", "general"])).unwrap();
        let expected = vec![Channel::General, Channel::Workspace("ws-1".into())];
        assert_eq!(d.channels, SecretaryChannels::Only { channels: expected });
        assert!(draft("m", "", &[], Some(&[])).is_err());
        assert!(draft("m", "", &[], Some(&["../x"])).is_err());
        assert!(draft("m", "", &[], Some(&["gone"])).is_err());
        let too_many: Vec<String> = (0..=CHANNELS_MAX).map(|i| format!("ws-{i}")).collect();
        let refs: Vec<&str> = too_many.iter().map(String::as_str).collect();
        assert!(draft("m", "", &[], Some(&refs)).is_err());
    }

    #[test]
    fn channels_and_tools_grant_only_what_was_chosen() {
        let ws = Channel::Workspace("ws-1".into());
        assert!(SecretaryChannels::All.allows(&Channel::General) && SecretaryChannels::All.allows(&ws));
        let only = SecretaryChannels::Only { channels: vec![Channel::General] };
        assert!(only.allows(&Channel::General) && !only.allows(&ws));
        assert!(!SecretaryChannels::Only { channels: vec![] }.allows(&Channel::General));
        let s: Secretary = serde_json::from_str(r#"{"name":"mario","tools":["thread_read"]}"#).unwrap();
        assert!(s.may_use(SecretaryTool::ThreadRead) && !s.may_use(SecretaryTool::BoardStatus));
    }

    #[test]
    fn channels_serialize_with_a_mode() {
        let all = serde_json::to_value(SecretaryChannels::All).unwrap();
        assert_eq!(all, serde_json::json!({ "mode": "all" }));
        let only = SecretaryChannels::Only { channels: vec![Channel::General] };
        assert_eq!(serde_json::to_value(only).unwrap(), serde_json::json!({ "mode": "only", "channels": ["general"] }));
    }

    #[test]
    fn unknown_tools_and_channel_modes_from_a_future_file_keep_the_rest() {
        let raw = r#"{"name":"mario","system_prompt":"Be brief.",
            "tools":["board_status","ask_user","post_message"],
            "channels":{"mode":"pinned","channels":["general"]}}"#;
        let s: Secretary = serde_json::from_str(raw).unwrap();
        assert_eq!(s.name, "mario");
        assert_eq!(s.system_prompt, "Be brief.");
        assert_eq!(s.tools, vec![SecretaryTool::BoardStatus, SecretaryTool::PostMessage]);
        assert_eq!(s.channels, SecretaryChannels::Only { channels: vec![] });
    }

    #[test]
    fn a_secretary_with_only_a_name_reads_with_defaults() {
        let s: Secretary = serde_json::from_str(r#"{"name":"mario"}"#).unwrap();
        assert_eq!(s.image, None);
        assert!(s.system_prompt.is_empty());
        assert!(s.tools.is_empty());
        assert_eq!(s.channels, SecretaryChannels::All);
        assert_eq!(s.preset, None);
    }

    #[test]
    fn the_preset_is_read_when_present_and_ignored_when_unreadable() {
        let s: Secretary = serde_json::from_str(r#"{"name":"mario","preset":"analyst"}"#).unwrap();
        assert_eq!(s.preset.as_deref(), Some("analyst"));
        let s: Secretary = serde_json::from_str(r#"{"name":"mario","preset":{"future":1}}"#).unwrap();
        assert_eq!(s.preset, None);
        let s: Secretary = serde_json::from_str(r#"{"name":"mario","preset":null}"#).unwrap();
        assert_eq!(s.preset, None);
    }
}
