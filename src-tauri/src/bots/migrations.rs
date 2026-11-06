//! Passi di migrazione di `bots.json`. Ogni passo e' congelato: descrive il
//! formato della sua versione e non cambia quando cambiano i tipi o i preset
//! correnti (per quello serve un passo nuovo in coda).
//!
//! I Bot predefiniti arrivano con passi additivi: ognuno aggiunge i suoi in
//! fondo a `agents`, salta gli id gia' presenti e non tocca nient'altro. Gira
//! una volta sola, quindi un Bot cancellato dall'utente non ricompare.

use serde_json::{json, Value};

/// Campo dei Bot-agente nel dato.
const AGENTS_KEY: &str = "agents";

/// Passo 1 -> 2: i quattro Bot predefiniti (`enabled`, `names`, `images` restano).
pub fn add_agent_bots(value: Value) -> Result<Value, String> {
    let bot = |id: &str, name: &str, task: &str, model: Value, effort: Value, skill: Value| {
        json!({
            "id": id, "name": name, "task": task, "agent": null, "model": model, "effort": effort,
            "skill": skill, "enabled": true, "image": null, "preset": id,
        })
    };
    let plugin = |id: &str| json!({"kind": "plugin", "id": id});
    let tier = |tier: &str| json!({"kind": "tier", "value": tier});
    add_missing(
        value,
        vec![
            bot(
                "general",
                "General",
                "General-purpose coding agent.",
                json!({"kind": "default"}),
                Value::Null,
                Value::Null,
            ),
            bot(
                "intake",
                "Intake",
                "Turns requests into queue tasks.",
                tier("strong"),
                json!("high"),
                plugin("task-intake"),
            ),
            bot(
                "setup",
                "Setup",
                "Configures the workspace and the worker rules.",
                tier("standard"),
                json!("medium"),
                plugin("project-setup"),
            ),
            bot(
                "skill-wizard",
                "Skill Wizard",
                "Creates and manages global skills.",
                tier("standard"),
                json!("medium"),
                plugin("skill-wizard"),
            ),
        ],
    )
}

/// Aggiunge in fondo a `agents` i Bot il cui id non c'e' ancora.
fn add_missing(mut value: Value, bots: Vec<Value>) -> Result<Value, String> {
    let data = value.as_object_mut().ok_or("il dato dei bot non è un oggetto")?;
    let agents = data.entry(AGENTS_KEY).or_insert_with(|| Value::Array(Vec::new()));
    let list = agents.as_array_mut().ok_or("il campo agents non è un elenco")?;
    for bot in bots {
        let id = bot.get("id").cloned();
        if !list.iter().any(|existing| existing.get("id") == id.as_ref()) {
            list.push(bot);
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::presets::default_bots;
    use crate::bots::{AgentBot, BotSettings};

    fn ids(value: &Value) -> Vec<&str> {
        value[AGENTS_KEY].as_array().unwrap().iter().map(|b| b["id"].as_str().unwrap()).collect()
    }

    #[test]
    fn the_v2_step_adds_the_four_presets_and_keeps_the_rest() {
        let v1 = json!({"enabled": false, "names": ["Ada"], "images": {"Ada": {"png": "AAAA", "color": "#112233"}}});
        let v2 = add_agent_bots(v1.clone()).unwrap();
        assert_eq!(ids(&v2), ["general", "intake", "setup", "skill-wizard"]);
        for key in ["enabled", "names", "images"] {
            assert_eq!(v2[key], v1[key], "{key}");
        }
    }

    /// Oggi il passo congelato e i preset coincidono. Se un preset cambia, il
    /// passo resta com'e' e questo test si aggiorna con il formato congelato.
    #[test]
    fn the_v2_step_matches_the_presets_of_its_release() {
        let v2 = add_agent_bots(json!({})).unwrap();
        let bots: Vec<AgentBot> = serde_json::from_value(v2[AGENTS_KEY].clone()).unwrap();
        assert_eq!(bots, default_bots());
        let settings: BotSettings = serde_json::from_value(v2).unwrap();
        assert_eq!(settings.agents, default_bots());
    }

    #[test]
    fn present_ids_are_skipped_and_others_are_untouched() {
        let mine = json!({"id": "intake", "name": "My intake"});
        let v2 = add_agent_bots(json!({"agents": [mine.clone()]})).unwrap();
        assert_eq!(ids(&v2), ["intake", "general", "setup", "skill-wizard"]);
        assert_eq!(v2[AGENTS_KEY][0], mine);
    }

    #[test]
    fn malformed_data_is_an_error() {
        assert!(add_agent_bots(json!([1, 2])).is_err());
        assert!(add_agent_bots(json!({"agents": "nope"})).is_err());
    }
}
