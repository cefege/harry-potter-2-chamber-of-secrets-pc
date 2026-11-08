//! Canonical native declarations for Engine trigger actors.

use crate::actor_owner::ACTOR_OPERATIONS;
use hp2_abi::engine::generated::{ATrigger, ATriggers};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, mem::offset_of};

const PUBLIC: u32 = 4;
const EDIT: u32 = 0x0000_0001;
const LOCALIZED: u32 = 0x0000_0040;
const OPERATIONS: ClassOps = ACTOR_OPERATIONS;

const TRIGGER_TYPE_VALUES: &[&str] = &[
    "TT_PlayerProximity", "TT_PawnProximity", "TT_ClassProximity", "TT_AnyProximity", "TT_Shoot",
];
const TELL_OTHER_TRIGGER_VALUES: &[&str] = &["TOT_TurnOn", "TOT_TurnOff"];

const fn property(name: &'static str, kind: StartupPropertyKind, property_flags: u32, offset: usize) -> StartupProperty {
    StartupProperty { name, kind, category: "", property_flags, object_flags: PUBLIC, inner_name: None, offset, array_dim: 1 }
}

const fn boolean(name: &'static str, bit: usize) -> StartupProperty {
    property(name, StartupPropertyKind::Bool { bit_mask: 1 << (bit % 32) }, EDIT, bit / 32 * 4)
}

const PROPERTIES: &[StartupProperty] = &[
    property("TriggerType", StartupPropertyKind::ByteEnum { name: "ETriggerType", values: TRIGGER_TYPE_VALUES }, EDIT, offset_of!(ATrigger, trigger_type)),
    property("Message", StartupPropertyKind::String, EDIT | LOCALIZED, offset_of!(ATrigger, message)),
    boolean("bTriggerOnceOnly", ATrigger::B_TRIGGER_ONCE_ONLY_BIT),
    boolean("bInitiallyActive", ATrigger::B_INITIALLY_ACTIVE_BIT),
    property("ClassProximityType", StartupPropertyKind::Class { meta_class: "Engine.Actor" }, EDIT, offset_of!(ATrigger, class_proximity_type)),
    property("RepeatTriggerTime", StartupPropertyKind::Float, EDIT, offset_of!(ATrigger, repeat_trigger_time)),
    property("ReTriggerDelay", StartupPropertyKind::Float, EDIT, offset_of!(ATrigger, re_trigger_delay)),
    property("TriggerTime", StartupPropertyKind::Float, 0, offset_of!(ATrigger, trigger_time)),
    property("DamageThreshold", StartupPropertyKind::Float, EDIT, offset_of!(ATrigger, damage_threshold)),
    boolean("bTT_CP_EventMustMatchTag", ATrigger::B_TT_CP_EVENT_MUST_MATCH_TAG_BIT),
    boolean("bSendEventOnEvent", ATrigger::B_SEND_EVENT_ON_EVENT_BIT),
    boolean("bDoActionWhenTriggered", ATrigger::B_DO_ACTION_WHEN_TRIGGERED_BIT),
    property("TellOtherTrigger", StartupPropertyKind::ByteEnum { name: "ETellOtherTrigger", values: TELL_OTHER_TRIGGER_VALUES }, EDIT, offset_of!(ATrigger, tell_other_trigger)),
    property("TriggerActor", StartupPropertyKind::Object { class: "Engine.Actor" }, 0, offset_of!(ATrigger, trigger_actor)),
    property("TriggerActor2", StartupPropertyKind::Object { class: "Engine.Actor" }, 0, offset_of!(ATrigger, trigger_actor2)),
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Triggers", package: "Engine", super_name: Some("Actor"), within: "Object",
        flags: 0, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<ATriggers>()), operations: Some(OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "Trigger", package: "Engine", super_name: Some("Triggers"), within: "Object",
        flags: 0, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<ATrigger>()), operations: Some(OPERATIONS), properties: PROPERTIES, defaults: &[],
    },
];
