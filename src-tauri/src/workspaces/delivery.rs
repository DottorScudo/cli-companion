//! Come un task arriva al repository (vedi docs/BOARD.MD, "Consegna"): e'
//! configurazione del workspace.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeliveryMode {
    /// Branch + push + PR + merge (`gh`).
    Pr,
    /// Branch + rebase + fast-forward della base locale.
    LocalMerge,
    /// Commit sul branch corrente + `pull --rebase` + push.
    Push,
    /// Solo commit sul branch corrente.
    #[default]
    Commit,
}

impl DeliveryMode {
    pub const ALL: [DeliveryMode; 4] =
        [DeliveryMode::Pr, DeliveryMode::LocalMerge, DeliveryMode::Push, DeliveryMode::Commit];

    /// Stesso valore della serializzazione.
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryMode::Pr => "pr",
            DeliveryMode::LocalMerge => "local_merge",
            DeliveryMode::Push => "push",
            DeliveryMode::Commit => "commit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MergeMethod {
    #[default]
    Squash,
    Merge,
    Rebase,
}

impl MergeMethod {
    pub const ALL: [MergeMethod; 3] = [MergeMethod::Squash, MergeMethod::Merge, MergeMethod::Rebase];

    pub fn as_str(self) -> &'static str {
        match self {
            MergeMethod::Squash => "squash",
            MergeMethod::Merge => "merge",
            MergeMethod::Rebase => "rebase",
        }
    }
}

/// Impostazione di consegna del workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Delivery {
    pub mode: DeliveryMode,
    /// Un worktree per task (solo `pr` e `local_merge`).
    pub parallel: bool,
    /// Solo per `pr`.
    pub merge_method: MergeMethod,
}

impl Delivery {
    /// La modalita' lavora su un branch `task/<id>`.
    pub fn uses_branches(&self) -> bool {
        matches!(self.mode, DeliveryMode::Pr | DeliveryMode::LocalMerge)
    }

    /// Ogni task ha il suo worktree (e i task possono andare in parallelo).
    pub fn worktrees(&self) -> bool {
        self.parallel && self.uses_branches()
    }

    /// `parallel` ha senso solo con i branch.
    pub(crate) fn normalized(mut self) -> Delivery {
        self.parallel &= self.uses_branches();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_normalization() {
        assert_eq!(
            Delivery::default(),
            Delivery { mode: DeliveryMode::Commit, parallel: false, merge_method: MergeMethod::Squash }
        );
        let d: Delivery = serde_json::from_str(r#"{"mode":"local_merge","parallel":true}"#).unwrap();
        assert!(d.worktrees() && d.uses_branches());
        assert_eq!(d.merge_method, MergeMethod::Squash);
        // `parallel` non ha senso senza branch.
        let push = Delivery { mode: DeliveryMode::Push, parallel: true, ..Default::default() };
        assert!(!push.worktrees() && !push.normalized().parallel);
        let pr: Delivery = serde_json::from_str(r#"{"mode":"pr","merge_method":"rebase"}"#).unwrap();
        assert!(pr.uses_branches() && !pr.worktrees());
        assert_eq!(serde_json::to_value(pr).unwrap()["mode"], "pr");
        assert!(serde_json::from_str::<Delivery>(r#"{"mode":"yolo"}"#).is_err());
    }

    #[test]
    fn names_match_the_serialization() {
        for mode in DeliveryMode::ALL {
            assert_eq!(serde_json::to_value(mode).unwrap(), mode.as_str());
        }
        for method in MergeMethod::ALL {
            assert_eq!(serde_json::to_value(method).unwrap(), method.as_str());
        }
    }
}
