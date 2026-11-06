//! Contratto delle skill del plugin: le sessioni guidate da una skill dei Bot.

use crate::agents::presets::Preset;

/// Skill del plugin (il catalogo e' del core: `agents::presets::PRESETS`).
///
/// Chi lo dichiara deve spedire nel proprio pacchetto la skill di OGNI voce di
/// `PRESETS`, al path `skill_path` (relativo, dentro `IntegrationPackage::files`):
/// un Bot con una skill del plugin la puo' usare solo con un agente che la dichiara.
pub trait PluginSkills: Sync {
    /// Dove il pacchetto spedisce la skill (letto dalla conformita').
    #[cfg_attr(not(test), expect(dead_code, reason = "read only by the conformance suite"))]
    fn skill_path(&self, skill: &Preset) -> String;

    /// Il nome con cui l'agente carica la skill `id` con il proprio strumento
    /// delle skill (e che il worker legge in `get_task`). Solo caratteri semplici
    /// (lettere, cifre e `-_.:`), senza prefissi di comando come `/`: la conformita' lo verifica.
    fn skill_name(&self, id: &str) -> String;
}
