//! Contratto `EffortLevels` di Claude Code: `--effort <livello>`.

use super::ClaudeCode;
use crate::agents::{Effort, EffortLevels};

/// Flag della CLI che fissa il livello di ragionamento.
const EFFORT_FLAG: &str = "--effort";

impl EffortLevels for ClaudeCode {
    fn levels(&self) -> &'static [Effort] {
        &Effort::ALL
    }

    /// Il testo viene dall'enum, mai da una stringa libera.
    fn command_args(&self, effort: Effort) -> Option<String> {
        Some(format!("{EFFORT_FLAG} {}", effort.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_is_declared_and_maps_to_its_flag() {
        assert_eq!(ClaudeCode.levels(), Effort::ALL);
        for effort in Effort::ALL {
            assert_eq!(ClaudeCode.command_args(effort), Some(format!("--effort {}", effort.as_str())));
        }
    }
}
