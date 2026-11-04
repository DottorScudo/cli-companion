//! Operazioni pure sull'elenco dei segretari: creare, modificare (anche
//! rinominare), eliminare, impostare o togliere l'immagine. Un'operazione che
//! fallisce non cambia nulla. I nomi si confrontano dopo `normalize_name`.

use super::secretary::{normalize_name, Secretary, SecretaryDraft};
use crate::bots::BotImage;

/// Segretari al massimo.
pub const SECRETARIES_MAX: usize = 20;

const NOT_FOUND: &str = "segretario non trovato";

/// Aggiunge un segretario in fondo, senza immagine.
pub fn create(list: &mut Vec<Secretary>, draft: SecretaryDraft) -> Result<(), String> {
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
