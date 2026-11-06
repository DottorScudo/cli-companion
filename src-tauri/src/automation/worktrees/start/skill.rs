//! La skill del Bot di un task, verificata prima dell'avvio: se non si puo'
//! caricare il task non parte e fallisce con una nota chiara, mai un ripiego
//! su un worker senza skill.

use crate::automation::task_skill;
use crate::automation::*;
use crate::board::Task;
use crate::bots::SkillRef;

/// Il Bot della copia: nel messaggio, per sapere cosa correggere.
fn bot_of(task: &Task) -> &str {
    task.bot.as_deref().unwrap_or("unknown")
}

/// Errore (in inglese, finisce nella nota del task) se la skill del task non si
/// puo' caricare: l'agente non sa farlo oppure la skill dell'utente non c'e' piu'.
pub(super) fn check<H: Host>(host: &H, task: &Task) -> Result<(), String> {
    let Some(skill) = &task.skill else {
        return Ok(());
    };
    if task_skill::invocation(task).is_none() {
        return Err(format!(
            "the skill {} of bot {} cannot be loaded by the agent {}: choose another bot",
            skill.id(),
            bot_of(task),
            task.agent
        ));
    }
    match skill {
        SkillRef::User(name) if !user_skill_exists(host, name)? => Err(format!(
            "the skill {name} of bot {} no longer exists: choose it again in the bots settings or change the task's bot",
            bot_of(task)
        )),
        _ => Ok(()),
    }
}

fn user_skill_exists<H: Host>(host: &H, name: &str) -> Result<bool, String> {
    let skills = crate::user_skills::list(&host.skills_store_dir()?)?;
    Ok(skills.iter().any(|s| s.name == name))
}
