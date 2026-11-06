//! Passi di migrazione di `bots.json`. Ogni passo e' congelato: descrive il
//! formato della sua versione e non cambia quando cambiano i tipi o i preset
//! correnti (per quello serve un passo nuovo in coda).
//!
//! I Bot predefiniti arrivano con passi additivi: ognuno aggiunge i suoi in
//! fondo a `agents` e salta gli id gia' presenti. Gira una volta sola, quindi
//! un Bot cancellato dall'utente non ricompare.

use serde_json::{json, Map, Value};

/// Campo dei Bot-agente nel dato.
const AGENTS_KEY: &str = "agents";
/// Campo dei tipi di task di un Bot (dalla v3).
const KINDS_KEY: &str = "kinds";
/// Tutti i tipi di task della v3.
const ALL_KINDS: [&str; 3] = ["work", "review", "plan"];
/// I Bot predefiniti della v2 con una skill interattiva: nessun task.
const INTERACTIVE_PRESETS: [&str; 3] = ["intake", "setup", "skill-wizard"];
/// Il Bot generico della v2: solo i task `work`.
const GENERAL: &str = "general";

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

/// Passo 2 -> 3: i tipi di task che ogni Bot puo' eseguire, poi i Bot Reviewer
/// e Planner. `general` esegue i task `work`, i preset con una skill interattiva
/// nessuno, tutti gli altri (derivati e dell'utente) ogni tipo, come prima. Il
/// nome di un Bot nuovo gia' usato prende il primo libero fra "Nome 2", "Nome 3"...
pub fn add_task_kinds(mut value: Value) -> Result<Value, String> {
    let list = agents_mut(&mut value)?;
    for bot in list.iter_mut() {
        let bot = bot.as_object_mut().ok_or("un Bot non è un oggetto")?;
        if !bot.contains_key(KINDS_KEY) {
            let kinds = v3_kinds(bot.get("id").and_then(Value::as_str));
            bot.insert(KINDS_KEY.to_string(), json!(kinds));
        }
    }
    let names: Vec<String> =
        list.iter().filter_map(|b| b.get("name").and_then(Value::as_str)).map(str::to_lowercase).collect();
    let bot = |id: &str, name: &str, task: &str, kind: &str| {
        json!({
            "id": id, "name": free_name(&names, name), "task": task, "agent": null,
            "model": {"kind": "tier", "value": "strong"}, "effort": "high", "skill": null, "kinds": [kind],
            "enabled": true, "image": null, "preset": id,
        })
    };
    let added = vec![
        bot("reviewer", "Reviewer", "Verifies the integrated result of finished tasks and proposes fixes.", "review"),
        bot("planner", "Planner", "Explores the repository read-only and proposes queue tasks.", "plan"),
    ];
    add_missing(value, added)
}

/// I tipi di task della v3 per un Bot della v2.
fn v3_kinds(id: Option<&str>) -> &'static [&'static str] {
    match id {
        Some(GENERAL) => &ALL_KINDS[..1],
        Some(id) if INTERACTIVE_PRESETS.contains(&id) => &[],
        _ => &ALL_KINDS,
    }
}

/// `base`, o `base N` col primo N libero fra `taken` (in minuscolo). I nomi
/// dei preset della v3 restano entro i 24 caratteri anche col suffisso.
fn free_name(taken: &[String], base: &str) -> String {
    let free = |name: &String| !taken.contains(&name.to_lowercase());
    std::iter::once(base.to_string())
        .chain((2..).map(|n| format!("{base} {n}")))
        .find(free)
        .expect("i suffissi numerici sono infiniti")
}

/// L'elenco `agents` del dato, creato vuoto se manca.
fn agents_mut(value: &mut Value) -> Result<&mut Vec<Value>, String> {
    let data: &mut Map<String, Value> = value.as_object_mut().ok_or("il dato dei bot non è un oggetto")?;
    let agents = data.entry(AGENTS_KEY).or_insert_with(|| Value::Array(Vec::new()));
    agents.as_array_mut().ok_or_else(|| "il campo agents non è un elenco".to_string())
}

/// Aggiunge in fondo a `agents` i Bot il cui id non c'e' ancora.
fn add_missing(mut value: Value, bots: Vec<Value>) -> Result<Value, String> {
    let list = agents_mut(&mut value)?;
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

    /// Oggi i passi congelati, uno dopo l'altro, e i preset coincidono. Se un
    /// preset cambia, i passi restano come sono e questo test si aggiorna con
    /// un passo nuovo in coda.
    #[test]
    fn the_steps_match_the_presets_of_the_current_release() {
        let v3 = add_task_kinds(add_agent_bots(json!({})).unwrap()).unwrap();
        let bots: Vec<AgentBot> = serde_json::from_value(v3[AGENTS_KEY].clone()).unwrap();
        assert_eq!(bots, default_bots());
        let settings: BotSettings = serde_json::from_value(v3).unwrap();
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
        for step in [add_agent_bots, add_task_kinds] {
            assert!(step(json!([1, 2])).is_err());
            assert!(step(json!({"agents": "nope"})).is_err());
        }
        assert!(add_task_kinds(json!({"agents": [1]})).is_err());
    }

    fn kinds_of<'a>(value: &'a Value, id: &str) -> &'a Value {
        let list = value[AGENTS_KEY].as_array().unwrap();
        &list.iter().find(|b| b["id"] == id).unwrap()[KINDS_KEY]
    }

    #[test]
    fn the_v3_step_gives_each_kind_of_bot_its_task_kinds() {
        let bot = |id: &str| json!({"id": id, "name": id});
        let ids_v2 = ["general", "intake", "setup", "skill-wizard", "intake-other", "custom-mine"];
        let v2 = json!({"agents": ids_v2.map(bot)});
        let v3 = add_task_kinds(v2).unwrap();
        assert_eq!(kinds_of(&v3, "general"), &json!(["work"]));
        for id in ["intake", "setup", "skill-wizard"] {
            assert_eq!(kinds_of(&v3, id), &json!([]), "{id}");
        }
        // Derivati e Bot dell'utente: nessun cambio di comportamento.
        for id in ["intake-other", "custom-mine"] {
            assert_eq!(kinds_of(&v3, id), &json!(ALL_KINDS), "{id}");
        }
        assert_eq!(kinds_of(&v3, "reviewer"), &json!(["review"]));
        assert_eq!(kinds_of(&v3, "planner"), &json!(["plan"]));
    }

    #[test]
    fn the_v3_step_keeps_the_rest_and_does_not_bring_back_deleted_presets() {
        let v2 = json!({"enabled": false, "names": ["Ada"], "agents": [{"id": "general", "name": "General"}]});
        let v3 = add_task_kinds(v2.clone()).unwrap();
        assert_eq!(ids(&v3), ["general", "reviewer", "planner"]);
        assert_eq!((&v3["enabled"], &v3["names"]), (&v2["enabled"], &v2["names"]));
        assert_eq!(v3[AGENTS_KEY][0]["name"], "General");
    }

    #[test]
    fn the_v3_step_skips_present_ids_and_frees_taken_names() {
        let mine = json!({"id": "planner", "name": "Mine", "kinds": ["work"]});
        let v2 = json!({"agents": [mine.clone(), {"id": "custom-r", "name": "reviewer"}, {"id": "custom-r2", "name": "Reviewer 2"}]});
        let v3 = add_task_kinds(v2).unwrap();
        assert_eq!(ids(&v3), ["planner", "custom-r", "custom-r2", "reviewer"]);
        assert_eq!(v3[AGENTS_KEY][0], mine, "kinds already set are kept");
        assert_eq!(v3[AGENTS_KEY][3]["name"], "Reviewer 3");
    }
}
