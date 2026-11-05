//! I segretari predefiniti (docs/HUB.MD): mansioni distinte, divise per il
//! modo di lavorare con l'utente e non per competenza tecnica. Aggiungerne uno =
//! una voce in `PRESETS` + un file in `presets/<id>.md` (il system prompt, in
//! inglese: lo legge il modello; le regole comuni le aggiunge il motore, vedi
//! `ENGINE_INSTRUCTIONS`). L'id e' stabile: lo ricorda il segretario creato da
//! un preset (`Secretary::preset`) e permette di ripristinarlo.

use super::secretary::{SecretaryChannels, SecretaryDraft, SecretaryTool};
use serde::Serialize;
use ts_rs::TS;

/// Un segretario predefinito.
pub struct SecretaryPreset {
    pub id: &'static str,
    /// Etichetta mostrata nell'interfaccia.
    pub label: &'static str,
    /// La mansione, in una riga, per l'interfaccia.
    pub description: &'static str,
    /// Nome suggerito alla creazione.
    pub name: &'static str,
    pub system_prompt: &'static str,
    pub tools: &'static [SecretaryTool],
    /// Si crea da solo alla prima attivazione dell'HUB (`secretaries::seed_presets`).
    /// Un preset che cambia la board no: lo sceglie l'utente.
    pub seeded: bool,
}

impl SecretaryPreset {
    /// System prompt con fine riga LF, qualunque sia il checkout.
    pub fn prompt_text(&self) -> String {
        self.system_prompt.replace("\r\n", "\n").trim().to_string()
    }

    /// I campi di un segretario nuovo creato da questo preset, in tutti i canali.
    pub fn draft(&self) -> SecretaryDraft {
        SecretaryDraft {
            name: self.name.to_string(),
            system_prompt: self.prompt_text(),
            tools: self.tools.to_vec(),
            channels: SecretaryChannels::All,
        }
    }
}

/// I preset, nell'ordine in cui si propongono e si creano.
///
/// Ogni preset elenca i propri tool in modo esplicito, mai `SecretaryTool::ALL`:
/// un tool nuovo non deve entrare da solo in un preset. In particolare i tool che
/// modificano la board (cambiano o eliminano task) non vanno in un preset senza
/// una scelta esplicita; il test `each_preset_has_the_tools_of_its_job` fissa le liste.
pub const PRESETS: [SecretaryPreset; 4] = [
    SecretaryPreset {
        id: "coordinator",
        label: "Coordinatore",
        description: "Trasforma i tuoi obiettivi in lavoro per gli agenti: apre i task di pianificazione e segue le proposte.",
        name: "mario",
        system_prompt: include_str!("presets/coordinator.md"),
        tools: &[
            SecretaryTool::BoardStatus,
            SecretaryTool::BoardOverview,
            SecretaryTool::TaskDetails,
            SecretaryTool::ThreadRead,
            SecretaryTool::PostMessage,
            SecretaryTool::OpenPlanTask,
            SecretaryTool::DecideProposal,
            SecretaryTool::RelayAnswer,
        ],
        seeded: true,
    },
    SecretaryPreset {
        id: "analyst",
        label: "Analista",
        description: "Ti fa le domande giuste su un’idea vaga, ne riassume la richiesta e solo dopo la tua conferma apre la pianificazione.",
        name: "giulia",
        system_prompt: include_str!("presets/analyst.md"),
        tools: &[SecretaryTool::ThreadRead, SecretaryTool::PostMessage, SecretaryTool::OpenPlanTask],
        seeded: true,
    },
    SecretaryPreset {
        id: "reporter",
        label: "Cronista",
        description: "Ti tiene informato su com’è la situazione: cosa è fatto, cosa gira, cosa aspetta te, cosa è fallito. Non apre né decide nulla.",
        name: "luca",
        system_prompt: include_str!("presets/reporter.md"),
        tools: &[
            SecretaryTool::BoardStatus,
            SecretaryTool::BoardOverview,
            SecretaryTool::TaskDetails,
            SecretaryTool::ThreadRead,
            SecretaryTool::PostMessage,
        ],
        seeded: true,
    },
    SecretaryPreset {
        id: "operator",
        label: "Operatore",
        description: "Agisce sulla coda quando glielo chiedi: ritenta i task falliti, cambia l’ordine, mette in pausa o riprende la board e ne regola il parallelismo. Può proporti di fermare, eliminare, chiudere o modificare un task: lo fai tu con un clic.",
        name: "ugo",
        system_prompt: include_str!("presets/operator.md"),
        tools: &[
            SecretaryTool::BoardStatus,
            SecretaryTool::BoardOverview,
            SecretaryTool::TaskDetails,
            SecretaryTool::ThreadRead,
            SecretaryTool::PostMessage,
            SecretaryTool::TaskAction,
            SecretaryTool::BoardControl,
            SecretaryTool::RequestTaskAction,
            SecretaryTool::RequestTaskUpdate,
        ],
        seeded: false,
    },
];

/// Il preset con quell'id.
pub fn find(id: &str) -> Option<&'static SecretaryPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Un preset per il frontend, che non duplica i testi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "HubSecretaryPreset")]
pub struct SecretaryPresetView {
    pub id: String,
    pub label: String,
    pub description: String,
    pub name: String,
    pub system_prompt: String,
    pub tools: Vec<SecretaryTool>,
}

pub fn views() -> Vec<SecretaryPresetView> {
    PRESETS
        .iter()
        .map(|p| SecretaryPresetView {
            id: p.id.to_string(),
            label: p.label.to_string(),
            description: p.description.to_string(),
            name: p.name.to_string(),
            system_prompt: p.prompt_text(),
            tools: p.tools.to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::settings::secretary::{normalize_name, SYSTEM_PROMPT_MAX};
    use std::collections::HashSet;

    #[test]
    fn the_presets_come_in_order_with_their_suggested_names() {
        let ids: Vec<_> = PRESETS.iter().map(|p| (p.id, p.name)).collect();
        assert_eq!(ids, [("coordinator", "mario"), ("analyst", "giulia"), ("reporter", "luca"), ("operator", "ugo")]);
    }

    #[test]
    fn only_the_presets_that_do_not_change_the_board_are_seeded() {
        let seeded: Vec<_> = PRESETS.iter().filter(|p| p.seeded).map(|p| p.id).collect();
        assert_eq!(seeded, ["coordinator", "analyst", "reporter"]);
        for p in PRESETS.iter().filter(|p| p.seeded) {
            for tool in [
                SecretaryTool::TaskAction,
                SecretaryTool::BoardControl,
                SecretaryTool::RequestTaskAction,
                SecretaryTool::RequestTaskUpdate,
            ] {
                assert!(!p.tools.contains(&tool), "{} {tool:?}", p.id);
            }
        }
    }

    #[test]
    fn ids_and_names_are_unique_and_names_are_valid() {
        let ids: HashSet<_> = PRESETS.iter().map(|p| p.id).collect();
        let names: HashSet<_> = PRESETS.iter().map(|p| p.name).collect();
        assert_eq!((ids.len(), names.len()), (PRESETS.len(), PRESETS.len()));
        for p in &PRESETS {
            assert_eq!(normalize_name(p.name).unwrap(), p.name);
        }
    }

    #[test]
    fn prompts_are_short_english_text_without_the_engine_rules() {
        for p in &PRESETS {
            let text = p.prompt_text();
            assert!(text.starts_with("Your job is"), "{}", p.id);
            assert!(text.chars().count() < SYSTEM_PROMPT_MAX / 4, "{}", p.id);
            assert!(!text.contains('\r') && text.is_ascii(), "{}", p.id);
            assert!(!text.contains("workspace id"), "{}", p.id);
        }
    }

    #[test]
    fn each_preset_has_the_tools_of_its_job() {
        use SecretaryTool::*;
        assert_eq!(
            find("coordinator").unwrap().tools,
            [
                BoardStatus,
                BoardOverview,
                TaskDetails,
                ThreadRead,
                PostMessage,
                OpenPlanTask,
                DecideProposal,
                RelayAnswer
            ]
        );
        assert_eq!(find("analyst").unwrap().tools, [ThreadRead, PostMessage, OpenPlanTask]);
        assert_eq!(find("reporter").unwrap().tools, [BoardStatus, BoardOverview, TaskDetails, ThreadRead, PostMessage]);
        assert_eq!(
            find("operator").unwrap().tools,
            [
                BoardStatus,
                BoardOverview,
                TaskDetails,
                ThreadRead,
                PostMessage,
                TaskAction,
                BoardControl,
                RequestTaskAction,
                RequestTaskUpdate
            ]
        );
        assert!(find("unknown").is_none());
    }

    #[test]
    fn views_carry_every_text_for_the_interface() {
        let views = views();
        assert_eq!(views.len(), PRESETS.len());
        assert_eq!(views[1].id, "analyst");
        assert_eq!(views[1].name, "giulia");
        assert_eq!(views[1].system_prompt, PRESETS[1].prompt_text());
        assert!(views.iter().all(|v| !v.label.is_empty() && !v.description.is_empty()));
    }
}
