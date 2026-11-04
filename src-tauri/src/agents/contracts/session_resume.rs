//! Contratto di ripristino: riprendere la conversazione di una sessione dopo
//! il riavvio dell'app.

/// Come l'agente riprende una sessione precedente.
pub trait SessionResume: Sync {
    /// `true` se la riga di comando avvia questo agente.
    #[cfg_attr(not(test), expect(dead_code, reason = "wired by a later task (agents-frontend)"))]
    fn is_agent_command(&self, command: &str) -> bool;

    /// Comando che riprende la sessione `session_id` al posto di quello originale.
    #[cfg_attr(not(test), expect(dead_code, reason = "wired by a later task (agents-frontend)"))]
    fn resume_command(&self, original_command: &str, session_id: &str) -> String;
}
