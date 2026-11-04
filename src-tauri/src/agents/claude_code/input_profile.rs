//! Contratto `InputProfileSource` di Claude Code.

use super::ClaudeCode;
use crate::agents::{InputEvent, InputProfile, InputProfileSource, InputRule};
use crate::platform::TargetKind;

impl InputProfileSource for ClaudeCode {
    /// Il tasto per incollare un'immagine e' Alt+V su Windows nativo
    /// ("Ctrl+V (Alt+V su Windows e WSL)"; su WSL valgono entrambi): xterm
    /// lo invia come ESC v.
    fn input_profile(&self) -> InputProfile {
        let rule =
            |env_kind| InputRule { event: InputEvent::ImagePaste, env_kind: Some(env_kind), send: "\x1bv".to_string() };
        InputProfile { rules: vec![rule(TargetKind::Windows), rule(TargetKind::Wsl)] }
    }
}
