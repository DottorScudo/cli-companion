//! Operazioni pure sull'elenco dei segretari: creare, modificare (anche
//! rinominare), eliminare, impostare o togliere l'immagine. Un'operazione che
//! fallisce non cambia nulla. I nomi si confrontano dopo `normalize_name`.

use super::presets::{self, PRESETS};
use super::secretary::{normalize_name, Secretary, SecretaryDraft};
use crate::bots::BotImage;

/// Segretari al massimo.
pub const SECRETARIES_MAX: usize = 20;

const NOT_FOUND: &str = "segretario non trovato";
const PRESET_NOT_FOUND: &str = "modello non trovato";
const NOT_FROM_PRESET: &str = "questo segretario non nasce da un modello";

/// Aggiunge un segretario in fondo, senza immagine.
pub fn create(list: &mut Vec<Secretary>, draft: SecretaryDraft) -> Result<(), String> {
    push(list, draft, None)
}

/// Come `create`, ricordando il preset `preset` da cui nasce.
pub fn create_from_preset(list: &mut Vec<Secretary>, draft: SecretaryDraft, preset: &str) -> Result<(), String> {
    let preset = presets::find(preset).ok_or_else(|| PRESET_NOT_FOUND.to_string())?;
    push(list, draft, Some(preset.id.to_string()))
}

/// Alla prima attivazione dell'HUB: se non c'è nessun segretario, crea i
/// preset `seeded` con i nomi suggeriti. Con un elenco già avviato non aggiunge
/// nulla. Un preset che non si può creare (nome già preso, tetto raggiunto) si salta.
pub fn seed_presets(list: &mut Vec<Secretary>) {
    if !list.is_empty() {
        return;
    }
    for preset in PRESETS.iter().filter(|p| p.seeded) {
        if let Err(reason) = create_from_preset(list, preset.draft(), preset.id) {
            log::info!("hub: il segretario predefinito {} non è stato creato: {reason}", preset.id);
        }
    }
}

/// Riporta prompt e tool del segretario `name` a quelli del suo preset; nome,
/// immagine e canali restano.
pub fn restore_preset(list: &mut [Secretary], name: &str) -> Result<(), String> {
    let at = position(list, name)?;
    let id = list[at].preset.as_deref().ok_or_else(|| NOT_FROM_PRESET.to_string())?;
    let preset = presets::find(id).ok_or_else(|| PRESET_NOT_FOUND.to_string())?;
    let secretary = &mut list[at];
    secretary.system_prompt = preset.prompt_text();
    secretary.tools = preset.tools.to_vec();
    Ok(())
}

fn push(list: &mut Vec<Secretary>, draft: SecretaryDraft, preset: Option<String>) -> Result<(), String> {
    if list.len() >= SECRETARIES_MAX {
        return Err(format!("al massimo {SECRETARIES_MAX} segretari"));
    }
    ensure_free(list, &draft.name, None)?;
    list.push(Secretary {
        name: draft.name,
        image: None,
        system_prompt: draft.system_prompt,
        tools: draft.tools,
        channels: draft.channels,
        preset,
    });
    Ok(())
}

/// Sostituisce i campi del segretario `name` (l'immagine resta). Rinominarlo
/// e' rifiutato se il nuovo nome e' di un altro segretario.
pub fn update(list: &mut [Secretary], name: &str, draft: SecretaryDraft) -> Result<(), String> {
    let at = position(list, name)?;
    ensure_free(list, &draft.name, Some(at))?;
    let secretary = &mut list[at];
    secretary.name = draft.name;
    secretary.system_prompt = draft.system_prompt;
    secretary.tools = draft.tools;
    secretary.channels = draft.channels;
    Ok(())
}

pub fn delete(list: &mut Vec<Secretary>, name: &str) -> Result<(), String> {
    let at = position(list, name)?;
    list.remove(at);
    Ok(())
}

/// Imposta (`Some`) o toglie (`None`) l'immagine del segretario `name`.
pub fn set_image(list: &mut [Secretary], name: &str, image: Option<BotImage>) -> Result<(), String> {
    let at = position(list, name)?;
    list[at].image = image;
    Ok(())
}

fn position(list: &[Secretary], name: &str) -> Result<usize, String> {
    let wanted = normalize_name(name).map_err(|_| NOT_FOUND.to_string())?;
    list.iter().position(|s| s.name == wanted).ok_or_else(|| NOT_FOUND.to_string())
}

/// Il nome non e' di nessun altro segretario (`except` = chi viene modificato).
fn ensure_free(list: &[Secretary], name: &str, except: Option<usize>) -> Result<(), String> {
    match list.iter().position(|s| s.name == name) {
        Some(i) if Some(i) != except => Err(format!("c'è già un segretario di nome “{name}”")),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::secretary::{SecretaryChannels, SecretaryTool};
    use super::*;

    fn draft(name: &str) -> SecretaryDraft {
        SecretaryDraft {
            name: name.to_string(),
            system_prompt: format!("You are {name}."),
            tools: vec![SecretaryTool::BoardStatus],
            channels: SecretaryChannels::All,
        }
    }

    fn list_of(names: &[&str]) -> Vec<Secretary> {
        let mut list = Vec::new();
        for n in names {
            create(&mut list, draft(n)).unwrap();
        }
        list
    }

    fn names(list: &[Secretary]) -> Vec<&str> {
        list.iter().map(|s| s.name.as_str()).collect()
    }

    fn image() -> BotImage {
        BotImage { png: "AAAA".into(), color: "#112233".into() }
    }

    #[test]
    fn create_appends_without_an_image() {
        let list = list_of(&["mario", "giorgio"]);
        assert_eq!(names(&list), ["mario", "giorgio"]);
        assert_eq!(list[0].image, None);
        assert_eq!(list[0].system_prompt, "You are mario.");
    }

    #[test]
    fn create_rejects_duplicates_and_caps_the_list() {
        let mut list = list_of(&["mario"]);
        assert!(create(&mut list, draft("mario")).is_err());
        let all: Vec<String> = (0..SECRETARIES_MAX).map(|i| format!("s{i}")).collect();
        let mut full = list_of(&all.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(full.len(), SECRETARIES_MAX);
        assert!(create(&mut full, draft("extra")).is_err());
        assert_eq!(full.len(), SECRETARIES_MAX);
    }

    #[test]
    fn update_replaces_the_fields_and_keeps_the_image() {
        let mut list = list_of(&["mario"]);
        set_image(&mut list, "mario", Some(image())).unwrap();
        let next = SecretaryDraft { system_prompt: String::new(), tools: vec![], ..draft("mario") };
        update(&mut list, " Mario ", next).unwrap();
        assert!(list[0].system_prompt.is_empty());
        assert!(list[0].tools.is_empty());
        assert_eq!(list[0].image, Some(image()));
    }

    #[test]
    fn rename_is_allowed_unless_the_name_is_taken() {
        let mut list = list_of(&["mario", "giorgio"]);
        assert!(update(&mut list, "mario", draft("giorgio")).is_err());
        assert_eq!(names(&list), ["mario", "giorgio"]);
        update(&mut list, "mario", draft("luigi")).unwrap();
        assert_eq!(names(&list), ["luigi", "giorgio"]);
        update(&mut list, "luigi", draft("luigi")).unwrap();
    }

    #[test]
    fn missing_secretaries_are_reported() {
        let mut list = list_of(&["mario"]);
        assert_eq!(update(&mut list, "anna", draft("anna")).unwrap_err(), NOT_FOUND);
        assert_eq!(delete(&mut list, "anna").unwrap_err(), NOT_FOUND);
        assert_eq!(set_image(&mut list, "bad name!", None).unwrap_err(), NOT_FOUND);
        assert_eq!(names(&list), ["mario"]);
    }

    fn seeded() -> Vec<Secretary> {
        let mut list = Vec::new();
        seed_presets(&mut list);
        list
    }

    #[test]
    fn an_empty_list_gets_the_three_presets() {
        let list = seeded();
        assert_eq!(names(&list), ["mario", "giulia", "luca"]);
        let presets: Vec<_> = list.iter().map(|s| s.preset.as_deref()).collect();
        assert_eq!(presets, [Some("coordinator"), Some("analyst"), Some("reporter")]);
        assert_eq!(list[0].tools, PRESETS[0].tools);
        assert!(list.iter().all(|s| !s.may_use(SecretaryTool::TaskAction) && !s.may_use(SecretaryTool::BoardControl)));
        assert_eq!(list[1].tools, [SecretaryTool::ThreadRead, SecretaryTool::PostMessage, SecretaryTool::OpenPlanTask]);
        assert!(list.iter().all(|s| s.is_configured() && s.channels == SecretaryChannels::All && s.image.is_none()));
    }

    #[test]
    fn a_list_that_already_has_secretaries_gets_nothing() {
        let mut list = list_of(&["mario"]);
        seed_presets(&mut list);
        assert_eq!(names(&list), ["mario"]);
        assert_eq!(list[0].system_prompt, "You are mario.");
        assert_eq!(list[0].preset, None);
    }

    #[test]
    fn seeding_twice_adds_nothing() {
        let mut list = seeded();
        seed_presets(&mut list);
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn a_taken_name_makes_create_from_preset_fail_without_changing_the_list() {
        let mut list = list_of(&["giulia"]);
        let err = create_from_preset(&mut list, PRESETS[1].draft(), "analyst").unwrap_err();
        assert!(err.contains("giulia"));
        assert_eq!(names(&list), ["giulia"]);
        assert_eq!(list[0].preset, None);
    }

    #[test]
    fn seeding_skips_a_preset_that_cannot_be_created_without_failing() {
        // Con il tetto già raggiunto nessun preset entra; la funzione non fallisce.
        let mut list: Vec<Secretary> = Vec::new();
        for i in 0..SECRETARIES_MAX {
            create(&mut list, draft(&format!("s{i}"))).unwrap();
        }
        seed_presets(&mut list);
        assert_eq!(list.len(), SECRETARIES_MAX);
    }

    #[test]
    fn create_from_preset_remembers_the_preset_and_rejects_unknown_ones() {
        let mut list = Vec::new();
        create_from_preset(&mut list, PRESETS[2].draft(), "reporter").unwrap();
        assert_eq!(list[0].preset.as_deref(), Some("reporter"));
        assert_eq!(create_from_preset(&mut list, draft("x"), "gone").unwrap_err(), PRESET_NOT_FOUND);
        assert_eq!(names(&list), ["luca"]);
        create(&mut list, draft("mario")).unwrap();
        assert_eq!(list[1].preset, None);
    }

    #[test]
    fn update_keeps_the_preset_of_an_edited_secretary() {
        let mut list = seeded();
        update(&mut list, "mario", draft("capo")).unwrap();
        assert_eq!(list[0].preset.as_deref(), Some("coordinator"));
    }

    #[test]
    fn restore_brings_back_prompt_and_tools_but_keeps_name_channels_and_image() {
        let mut list = seeded();
        set_image(&mut list, "giulia", Some(image())).unwrap();
        let edited = SecretaryDraft {
            name: "gia".into(),
            system_prompt: "Something else.".into(),
            tools: vec![SecretaryTool::RelayAnswer],
            channels: SecretaryChannels::Only { channels: vec![] },
        };
        update(&mut list, "giulia", edited).unwrap();
        restore_preset(&mut list, "gia").unwrap();
        assert_eq!(list[1].name, "gia");
        assert_eq!(list[1].system_prompt, PRESETS[1].prompt_text());
        assert_eq!(list[1].tools, PRESETS[1].tools);
        assert_eq!(list[1].channels, SecretaryChannels::Only { channels: vec![] });
        assert_eq!(list[1].image, Some(image()));
        assert_eq!(list[1].preset.as_deref(), Some("analyst"));
    }

    #[test]
    fn the_operator_is_created_only_on_request_and_restored_with_its_board_tools() {
        let operator = presets::find("operator").unwrap();
        let mut list = seeded();
        assert!(list.iter().all(|s| s.preset.as_deref() != Some("operator")));
        create_from_preset(&mut list, operator.draft(), "operator").unwrap();
        let edited = SecretaryDraft { tools: vec![SecretaryTool::ThreadRead], ..operator.draft() };
        update(&mut list, "ugo", edited).unwrap();
        restore_preset(&mut list, "ugo").unwrap();
        let ugo = list.iter().find(|s| s.name == "ugo").unwrap();
        assert_eq!(ugo.preset.as_deref(), Some("operator"));
        assert_eq!(ugo.tools, operator.tools);
        assert!(ugo.may_use(SecretaryTool::TaskAction) && ugo.may_use(SecretaryTool::BoardControl));
        assert_eq!(ugo.system_prompt, operator.prompt_text());
    }

    #[test]
    fn restore_is_refused_without_a_known_preset() {
        let mut list = list_of(&["mario"]);
        assert_eq!(restore_preset(&mut list, "mario").unwrap_err(), NOT_FROM_PRESET);
        list[0].preset = Some("from-the-future".into());
        assert_eq!(restore_preset(&mut list, "mario").unwrap_err(), PRESET_NOT_FOUND);
        assert_eq!(restore_preset(&mut list, "anna").unwrap_err(), NOT_FOUND);
        assert_eq!(list[0].system_prompt, "You are mario.");
    }

    #[test]
    fn delete_and_image_removal() {
        let mut list = list_of(&["mario", "giorgio"]);
        set_image(&mut list, "giorgio", Some(image())).unwrap();
        set_image(&mut list, "giorgio", None).unwrap();
        assert_eq!(list[1].image, None);
        delete(&mut list, "MARIO").unwrap();
        assert_eq!(names(&list), ["giorgio"]);
    }
}
