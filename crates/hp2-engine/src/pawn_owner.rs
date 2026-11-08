//! Canonical Rust class lifecycle for Engine's Pawn actor family.

use crate::actor_owner;
use hp2_abi::engine::generated::{APawn, APlayerPawn};
use hp2_core::core_class_specs::{
    NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind,
};
use std::{alloc::Layout, mem};

const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_CONFIG: u32 = 0x0000_0004;
const CLASS_NATIVE_REPLICATION: u32 = 0x0000_0800;

const CPF_EDIT: u32 = 0x0000_0001;
const CPF_CONST: u32 = 0x0000_0002;
const CPF_INPUT: u32 = 0x0000_0004;
const CPF_TRAVEL: u32 = 0x0001_0000;
const CPF_EDIT_CONST: u32 = 0x0002_0000;
const CPF_CONFIG: u32 = 0x0000_4000;
const CPF_GLOBAL_CONFIG: u32 = 0x0004_0000;

const GAME_STATE_TOKEN_LEN_OFFSET: usize =
    mem::offset_of!(APlayerPawn, game_state_token_len);
const GAME_STATE_STRINGS_OFFSET: usize =
    mem::offset_of!(APlayerPawn, f_lumos_radius) - mem::size_of::<u32>();
const LUMOS_FLAGS_OFFSET: usize = GAME_STATE_STRINGS_OFFSET;
const SAVE_FLAGS_OFFSET: usize = mem::offset_of!(APlayerPawn, b_mouse_alt_fire) - mem::size_of::<u32>();
const PUBLIC: u32 = 4;

const PLAYER_PAWN_PROPERTIES: &[StartupProperty] = &[
    StartupProperty {
        name: "bShowLoadingScreen",
        kind: StartupPropertyKind::Bool { bit_mask: 1 },
        category: "",
        property_flags: CPF_TRAVEL,
        object_flags: PUBLIC,
        inner_name: None,
        offset: mem::offset_of!(APlayerPawn, total_game_state_tokens) - mem::size_of::<u32>(),
        array_dim: 1,
    },
    StartupProperty {
        name: "TotalGameStateTokens", kind: StartupPropertyKind::Int, category: "",
        property_flags: 0, object_flags: PUBLIC, inner_name: None,
        offset: mem::offset_of!(APlayerPawn, total_game_state_tokens), array_dim: 1,
    },
    StartupProperty {
        name: "GameStateTokenLen", kind: StartupPropertyKind::Int, category: "",
        property_flags: 0, object_flags: PUBLIC, inner_name: None,
        offset: GAME_STATE_TOKEN_LEN_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "GameStateMasterList", kind: StartupPropertyKind::String, category: "GameState",
        property_flags: CPF_EDIT | CPF_CONST | CPF_EDIT_CONST, object_flags: PUBLIC,
        inner_name: None, offset: mem::offset_of!(APlayerPawn, game_state_master_list), array_dim: 1,
    },
    StartupProperty {
        name: "CurrentGameState", kind: StartupPropertyKind::String, category: "GameState",
        property_flags: CPF_EDIT | CPF_TRAVEL, object_flags: PUBLIC,
        inner_name: None, offset: mem::offset_of!(APlayerPawn, current_game_state), array_dim: 1,
    },
    StartupProperty {
        name: "bLumosOn", kind: StartupPropertyKind::Bool { bit_mask: 1 }, category: "",
        property_flags: 0, object_flags: PUBLIC, inner_name: None, offset: LUMOS_FLAGS_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "fLumosRadius", kind: StartupPropertyKind::Float, category: "",
        property_flags: 0, object_flags: PUBLIC, inner_name: None,
        offset: mem::offset_of!(APlayerPawn, f_lumos_radius), array_dim: 1,
    },
    StartupProperty {
        name: "bQueuedToSaveGame", kind: StartupPropertyKind::Bool { bit_mask: 1 }, category: "",
        property_flags: 0, object_flags: PUBLIC, inner_name: None, offset: SAVE_FLAGS_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "bIsCaptured", kind: StartupPropertyKind::Bool { bit_mask: 2 }, category: "",
        property_flags: 0, object_flags: PUBLIC, inner_name: None, offset: SAVE_FLAGS_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "bModernThirdPersonControls", kind: StartupPropertyKind::Bool { bit_mask: 4 }, category: "",
        property_flags: CPF_CONFIG | CPF_GLOBAL_CONFIG, object_flags: PUBLIC,
        inner_name: None, offset: SAVE_FLAGS_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "bMouseAltFire", kind: StartupPropertyKind::Byte, category: "",
        property_flags: CPF_INPUT, object_flags: PUBLIC, inner_name: None,
        offset: mem::offset_of!(APlayerPawn, b_mouse_alt_fire), array_dim: 1,
    },
    StartupProperty {
        name: "bKeyAltFire", kind: StartupPropertyKind::Byte, category: "",
        property_flags: CPF_INPUT, object_flags: PUBLIC, inner_name: None,
        offset: mem::offset_of!(APlayerPawn, b_key_alt_fire), array_dim: 1,
    },
];
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Pawn",
        package: "Engine",
        super_name: Some("Actor"),
        within: "Object",
        flags: CLASS_CONFIG | CLASS_NATIVE_REPLICATION,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<APawn>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "PlayerPawn",
        package: "Engine",
        super_name: Some("Pawn"),
        within: "Object",
        flags: CLASS_CONFIG | CLASS_NATIVE_REPLICATION,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<APlayerPawn>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS),
        properties: PLAYER_PAWN_PROPERTIES,
        defaults: &[],
    },
];
