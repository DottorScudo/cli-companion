//! La riga di `get_task` che manda il worker a caricare la skill del suo Bot.

/// La riga che precede le istruzioni di un task con una skill. `invocation` e'
/// il nome con cui l'agente del worker la carica (vedi `agents::worker_skill_invocation`).
pub fn skill_instruction(invocation: &str) -> String {
    format!(
        "Before starting, load the skill {invocation} with your skill tool; the instructions of this task take precedence over the skill where they conflict."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_skill_and_keeps_the_task_in_charge() {
        assert_eq!(
            skill_instruction("cli-companion-notify:task-intake"),
            "Before starting, load the skill cli-companion-notify:task-intake with your skill tool; the instructions of this task take precedence over the skill where they conflict."
        );
    }
}
