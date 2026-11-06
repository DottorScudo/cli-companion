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
/// Campo della descrizione di un Bot.
const TASK_KEY: &str = "task";
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
    let names = bot_names(list);
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

/// Passo 3 -> 4: i Bot Quick Answer (domande sul codice, task `plan`) e Bug
/// Fixer (correzione di un bug, task `work`), con le skill `code-answer` e
/// `bug-fix`. Il nome gia' preso da un Bot prende il primo libero come nel
/// passo 2 -> 3; i nomi dei segretari non sono noti qui: li libera, dopo il
/// caricamento, il modulo `secretary_names`.
pub fn add_answer_and_fix_bots(mut value: Value) -> Result<Value, String> {
    let names = bot_names(agents_mut(&mut value)?);
    let tier = |tier: &str| json!({"kind": "tier", "value": tier});
    let bot = |id: &str, name: &str, task: &str, model: Value, effort: &str, skill: &str, kind: &str| {
        json!({
            "id": id, "name": free_name(&names, name), "task": task, "agent": null, "model": model,
            "effort": effort, "skill": {"kind": "plugin", "id": skill}, "kinds": [kind],
            "enabled": true, "image": null, "preset": id,
        })
    };
    let added = vec![
        bot(
            "quick-answer",
            "Quick Answer",
            "Answers a quick question about the code or its git history, read-only, in a few lines. Changes nothing \
             and proposes nothing.",
            tier("standard"),
            "low",
            "code-answer",
            "plan",
        ),
        bot(
            "bug-fixer",
            "Bug Fixer",
            "Fixes a reported bug: reproduces it, adds a failing regression test, fixes the root cause and verifies.",
            tier("strong"),
            "high",
            "bug-fix",
            "work",
        ),
    ];
    add_missing(value, added)
}

/// Passo 4 -> 5: i `task` di `general` e `planner` resi esclusivi rispetto a
/// Quick Answer e Bug Fixer. Cambia solo un testo ancora uguale al vecchio
/// preset: uno diverso e' una personalizzazione e resta com'e'.
pub fn exclusive_general_and_planner_tasks(mut value: Value) -> Result<Value, String> {
    /// Per id: il vecchio `task` del preset e quello nuovo.
    const UPDATES: [(&str, &str, &str); 2] = [
        (
            GENERAL,
            "General-purpose coding agent.",
            "General-purpose coding agent: new features and any change no specialized bot covers.",
        ),
        (
            "planner",
            "Explores the repository read-only and proposes queue tasks.",
            "Plans changes to the project: explores the repository read-only and proposes queue tasks.",
        ),
    ];
    for bot in agents_mut(&mut value)?.iter_mut() {
        let bot = bot.as_object_mut().ok_or("un Bot non è un oggetto")?;
        let id = bot.get("id").and_then(Value::as_str);
        let Some(&(_, old, new)) = UPDATES.iter().find(|(update_id, ..)| id == Some(*update_id)) else {
            continue;
        };
        if bot.get(TASK_KEY).and_then(Value::as_str) == Some(old) {
            bot.insert(TASK_KEY.to_string(), json!(new));
        }
    }
    Ok(value)
}

/// Passo 5 -> 6: i Bot Researcher (ricerca tecnica, task `plan`), Test Writer,
/// Docs Writer e Refactorer (task `work`), con le skill `code-research`,
/// `test-qa`, `docs-writer` e `safe-refactor`. Nomi liberi come nel passo 3 -> 4;
/// un nome di segretario lo libera `secretary_names` dopo il caricamento.
pub fn add_research_and_work_bots(mut value: Value) -> Result<Value, String> {
    let names = bot_names(agents_mut(&mut value)?);
    let tier = |tier: &str| json!({"kind": "tier", "value": tier});
    let bot = |id: &str, name: &str, task: &str, model: Value, effort: &str, skill: &str, kind: &str| {
        json!({
            "id": id, "name": free_name(&names, name), "task": task, "agent": null, "model": model,
            "effort": effort, "skill": {"kind": "plugin", "id": skill}, "kinds": [kind],
            "enabled": true, "image": null, "preset": id,
        })
    };
    let added = vec![
        bot(
            "researcher",
            "Researcher",
            "Researches a technical question in depth, read-only: options, feasibility, risks and effort, in a \
             structured report. May propose tasks only if the user asked for them.",
            tier("strong"),
            "high",
            "code-research",
            "plan",
        ),
        bot(
            "test-writer",
            "Test Writer",
            "Adds or improves automated tests for existing behaviour, following the project's test conventions; \
             changes production code only to make it testable.",
            tier("standard"),
            "medium",
            "test-qa",
            "work",
        ),
        bot(
            "docs-writer",
            "Docs Writer",
            "Writes or updates documentation (README, guides, comments) for the stated audience and language; does \
             not change behaviour.",
            tier("standard"),
            "medium",
            "docs-writer",
            "work",
        ),
        bot(
            "refactorer",
            "Refactorer",
            "Restructures existing code without changing its behaviour, in small verified steps, with tests run \
             before and after.",
            tier("strong"),
            "high",
            "safe-refactor",
            "work",
        ),
    ];
    add_missing(value, added)
}

/// Passo 6 -> 7: i Bot Git Operator (manutenzione git) e Release (preparazione
/// di un rilascio), task `work`, con le skill `git-ops` e `release-prep`.
/// Nascono **spenti**: i loro effetti escono dal flusso di consegna e l'utente li
/// accende di proposito. Nomi liberi come nel passo 3 -> 4; un nome di segretario
/// ("release") lo libera `secretary_names` dopo il caricamento.
pub fn add_git_and_release_bots(mut value: Value) -> Result<Value, String> {
    let names = bot_names(agents_mut(&mut value)?);
    let bot = |id: &str, name: &str, task: &str, effort: &str, skill: &str| {
        json!({
            "id": id, "name": free_name(&names, name), "task": task, "agent": null,
            "model": {"kind": "tier", "value": "strong"}, "effort": effort, "skill": {"kind": "plugin", "id": skill},
            "kinds": ["work"], "enabled": false, "image": null, "preset": id,
        })
    };
    let added = vec![
        bot(
            "git-operator",
            "Git Operator",
            "Performs git maintenance asked by the user (branch cleanup, conflict resolution, reverting a change) \
             safely: never rewrites shared history or force-pushes.",
            "medium",
            "git-ops",
        ),
        bot(
            "release",
            "Release",
            "Prepares a release: version bump, changelog and release notes from the merged work; never publishes, \
             tags remotely or deploys without the user's explicit confirmation.",
            "high",
            "release-prep",
        ),
    ];
    add_missing(value, added)
}

/// I nomi dei Bot dell'elenco, in minuscolo.
fn bot_names(list: &[Value]) -> Vec<String> {
    list.iter().filter_map(|b| b.get("name").and_then(Value::as_str)).map(str::to_lowercase).collect()
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
/// dei preset della v3, v4, v6 e v7 restano entro i 24 caratteri anche col suffisso.
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
mod tests;
