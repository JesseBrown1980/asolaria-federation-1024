//! C/D substrate room routing — clean-room port of D:/bigpickle-rebuild/src/project-room-router.mjs
//! (operator pattern, verbatim 2026-05-25). C: = the 10k ROTATING project rooms (rename-before-load
//! defeats the same-project throttle so each free-agent load looks like a NEW project = $0). D: = the
//! PRISM output (many-rooms -> reverse-gain GNN -> 1 answer). PURE / E=0: planners only — the executor
//! seam (atomic fs renames on C, prism writes on D) is the host8/caller's GATED responsibility. This
//! is the operator's "10k and 10k prisms flowing on C and D" expressed as a routing contract, not a
//! launcher: nothing here renames a folder, writes a file, or spawns a process.

use alloc::string::String;

/// Operator-canonical room count per substrate (project-room-router.mjs ROOM_COUNT).
pub const ROOM_COUNT: u32 = 10_000;

/// The two substrates the loop flows across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Substrate {
    /// C: — rotating project rooms where free-agents load fresh ($0 via rename-before-load).
    CDrive,
    /// D: — prism / gulp / cube output (reverse-gain aggregation, the district fabric).
    DDrive,
}

/// Loop stage, for C/D substrate placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomStage {
    /// A fresh agent project room — rotates on C:.
    AgentRoom,
    /// A prism / gulp / cube OUTPUT room — lands on D:.
    PrismOutput,
}

/// C/D placement: agent rooms rotate on C, prism/gulp/cube output lands on D (project-room-router:
/// "rotate folder names on C; those outputs route back into the system on D with a prism device").
pub fn substrate_for_stage(stage: RoomStage) -> Substrate {
    match stage {
        RoomStage::AgentRoom => Substrate::CDrive,
        RoomStage::PrismOutput => Substrate::DDrive,
    }
}

/// Deterministic PID -> room id (roomIdFromPid: 31-rolling-hash over the PID's bytes, mod ROOM_COUNT,
/// with `>>> 0` u32 wraparound). ASCII-parity with the node router (PIDs are ASCII).
pub fn room_id_from_pid(pid: &str) -> u32 {
    let mut acc: u32 = 0;
    for b in pid.bytes() {
        acc = acc.wrapping_mul(31).wrapping_add(b as u32);
    }
    acc % ROOM_COUNT
}

/// Deterministic room folder name (roomFolderName: `omni-room-behcs-256-NNNN`). The name a C-drive
/// rotation renames TO so each agent load looks like a NEW project (the throttle bypass).
pub fn room_folder_name(room_id: u32) -> String {
    alloc::format!("omni-room-behcs-256-{:04}", room_id % ROOM_COUNT)
}

/// Prism output path on the D: substrate (prismRoutePlan out_path: `<base>/<folder>/prism-out.ndjson`).
pub fn prism_out_path(room_id: u32, prism_base: &str) -> String {
    alloc::format!(
        "{}/{}/prism-out.ndjson",
        prism_base,
        room_folder_name(room_id)
    )
}

/// A rename-before-load room swap on C: (planRoomSwap). `None` = no-op (same room). PURE: the caller
/// performs the atomic fs rename; this only plans the from/to folder names + the next room id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomSwapPlan {
    pub from_folder: String,
    pub to_folder: String,
    pub next_room_id: u32,
}

/// Plan a rename-before-load swap from `current_room_id` to `next_room_id`. `None` if unchanged.
pub fn plan_room_swap(current_room_id: u32, next_room_id: u32) -> Option<RoomSwapPlan> {
    if current_room_id == next_room_id {
        return None;
    }
    Some(RoomSwapPlan {
        from_folder: room_folder_name(current_room_id),
        to_folder: room_folder_name(next_room_id),
        next_room_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_id_from_pid_matches_node_router() {
        // Parity-pinned against node project-room-router.mjs roomIdFromPid (via C:/tmp/room-parity.mjs).
        assert_eq!(room_id_from_pid("deadbeef"), 2488);
        assert_eq!(room_id_from_pid("AGT-ACER-HRM-H123456789AB"), 3863);
        assert_eq!(room_id_from_pid("0000000000000000"), 2432);
        assert_eq!(room_id_from_pid("ffffffffffffffff"), 6048);
        assert_eq!(room_id_from_pid("a"), 97);
    }

    #[test]
    fn room_id_is_bounded_to_room_count() {
        assert!(room_id_from_pid("any-pid-whatsoever-very-long-string-here") < ROOM_COUNT);
        assert_eq!(ROOM_COUNT, 10_000);
    }

    #[test]
    fn room_folder_name_is_zero_padded_4() {
        assert_eq!(room_folder_name(97), "omni-room-behcs-256-0097");
        assert_eq!(room_folder_name(2488), "omni-room-behcs-256-2488");
    }

    #[test]
    fn substrate_split_c_rooms_d_prism() {
        assert_eq!(substrate_for_stage(RoomStage::AgentRoom), Substrate::CDrive);
        assert_eq!(substrate_for_stage(RoomStage::PrismOutput), Substrate::DDrive);
    }

    #[test]
    fn prism_out_path_is_on_the_d_substrate() {
        assert_eq!(
            prism_out_path(2488, "D:/Asolaria-Districts/prism"),
            "D:/Asolaria-Districts/prism/omni-room-behcs-256-2488/prism-out.ndjson"
        );
    }

    #[test]
    fn room_swap_is_rename_before_load_and_noop_on_same() {
        assert_eq!(plan_room_swap(5, 5), None);
        let plan = plan_room_swap(5, 6).expect("swap");
        assert_eq!(plan.from_folder, "omni-room-behcs-256-0005");
        assert_eq!(plan.to_folder, "omni-room-behcs-256-0006");
        assert_eq!(plan.next_room_id, 6);
    }
}
