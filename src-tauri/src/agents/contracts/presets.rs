//! Contratto dei preset: sessioni interattive guidate da una skill.

use crate::agents::presets::Preset;

/// Avvio dei preset (il catalogo e' del core: `agents::presets::PRESETS`).
///
/// Chi lo dichiara deve spedire nel proprio pacchetto la skill di OGNI preset
/// di `PRESETS`, al path `skill_path` (relativo, dentro `IntegrationPackage::files`).
pub trait PresetLauncher: Sync {
    /// Comando che avvia l'agente con la skill del preset come primo prompt.
    /// Digitato nella shell del tab: valido in PowerShell, cmd e bash.
    fn command(&self, preset: &Preset) -> String;

    /// Dove il pacchetto spedisce la skill del preset (letto dalla conformita').
    #[cfg_attr(not(test), expect(dead_code, reason = "read only by the conformance suite"))]
    fn skill_path(&self, preset: &Preset) -> String;
}
