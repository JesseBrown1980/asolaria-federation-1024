//! Free-agent runner lane table — which provider/CLI + default model for each agent role. This is
//! the LANE SPEC only (PURE / E=0, a role->runner mapping): the operator's "100 OpenCode + 100
//! Hermes" expressed as a table. The actual launch (Command::spawn of the CLI into a fresh C-drive
//! room) is host8's GATED job behind &fire=1 + EXEC-FREEZE release. Nothing here spawns a process,
//! reads the environment, or touches the filesystem — it only NAMES the binary env var + model.

use crate::AgentRole;

/// Which free-agent provider/CLI a lane uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    /// OpenCode CLI — the proven $0 lane (`opencode run -m <model> --dir <fresh-room>`; fresh room
    /// per call via rename-before-load = free).
    OpenCode,
    /// Hermes coordinator lane.
    Hermes,
}

/// A runner lane spec: the provider, the env var that NAMES its binary, and the default model. The
/// launch site reads `bin_env` from the environment AT FIRE TIME (gated) — this struct only names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerSpec {
    pub kind: RunnerKind,
    /// Env var naming the CLI binary path (read by the gated launch site, not here).
    pub bin_env: &'static str,
    /// Default model for the lane when the caller does not override.
    pub default_model: &'static str,
}

/// Map an agent role to its runner lane. `Hermes` -> the Hermes lane; every other role -> the
/// OpenCode $0 lane (the proven default). PURE: no process, no env read, no fs.
pub fn runner_for_role(role: AgentRole) -> RunnerSpec {
    match role {
        AgentRole::Hermes => RunnerSpec {
            kind: RunnerKind::Hermes,
            bin_env: "ASOLARIA_HERMES_BIN",
            default_model: "hermes-4-70b",
        },
        _ => RunnerSpec {
            kind: RunnerKind::OpenCode,
            bin_env: "OPENCODE_BIN",
            default_model: "opencode/big-pickle",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermes_role_uses_hermes_lane() {
        let spec = runner_for_role(AgentRole::Hermes);
        assert_eq!(spec.kind, RunnerKind::Hermes);
        assert_eq!(spec.bin_env, "ASOLARIA_HERMES_BIN");
        assert_eq!(spec.default_model, "hermes-4-70b");
    }

    #[test]
    fn other_roles_use_the_opencode_dollar_zero_lane() {
        for role in [
            AgentRole::SubAgent,
            AgentRole::BigPickle,
            AgentRole::SpaceAgent,
            AgentRole::Pi,
            AgentRole::Omnispindle,
            AgentRole::Omniflywheel,
            AgentRole::SpaceDeckDriver,
        ] {
            let spec = runner_for_role(role);
            assert_eq!(
                spec.kind,
                RunnerKind::OpenCode,
                "role {role:?} should use OpenCode"
            );
            assert_eq!(spec.bin_env, "OPENCODE_BIN");
            assert_eq!(spec.default_model, "opencode/big-pickle");
        }
    }

    #[test]
    fn the_two_lanes_are_distinct() {
        assert_ne!(
            runner_for_role(AgentRole::Hermes).kind,
            runner_for_role(AgentRole::SubAgent).kind
        );
    }
}
