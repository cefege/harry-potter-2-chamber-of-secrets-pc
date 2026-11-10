//! Canonical checked and named Core VM native identities.
//!
//! The catalogue owns names and opcode identities; consumers supply handlers.

use hp2_abi::engine::generated;
use crate::vm_registry::NATIVE_COUNT;


/// Name and numeric dispatch identity of a Core native.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSpec {
    /// Stable native identity.
    pub native: CoreNative,
    /// Checked numeric index, or None for a name-only binding.
    pub opcode: Option<i32>,
    /// Exact legacy token spelling used by UnrealScript binding.
    pub token_name: &'static str,
}

/// Iterates checked and name-only Core natives in registration order.
#[must_use]
pub fn iter() -> impl Iterator<Item = &'static NativeSpec> { CORE_NATIVES.iter() }

/// Finds a Core native by exact token spelling.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static NativeSpec> {
    CORE_NATIVES.iter().find(|spec| spec.token_name == name)
}

/// Finds the checked Core native assigned to opcode.
#[must_use]
pub fn by_opcode(opcode: i32) -> Option<&'static NativeSpec> {
    // Opcodes are unique and below `NATIVE_COUNT` (checked at compile time where the
    // catalog is defined), so a direct table replaces a scan of the whole catalog on
    // every VM native call.
    static INDEX: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    let index = INDEX.get_or_init(|| {
        let mut table = vec![u16::MAX; NATIVE_COUNT];
        for (position, spec) in CORE_NATIVES.iter().enumerate() {
            if let Some(code) = spec.opcode {
                table[code as usize] = u16::try_from(position).expect("catalog fits u16");
            }
        }
        table
    });
    let position = *index.get(usize::try_from(opcode).ok()?)?;
    (position != u16::MAX).then(|| &CORE_NATIVES[usize::from(position)])
}

macro_rules! native_opcode {
    ($name:ident) => { generated::$name };
}

macro_rules! define_catalog {
    (checked { $(($opcode:expr, $token:ident)),* $(,)? } named { $(($named_token:ident)),* $(,)? }) => {
        /// Typed identities for the canonical Core native list.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        #[allow(non_camel_case_types)]
        pub enum CoreNative { $($token,)* $($named_token,)* }

        /// All Core native identities, in canonical registration order.
        pub static CORE_NATIVES: &[NativeSpec] = &[
            $(NativeSpec { native: CoreNative::$token, opcode: Some($opcode), token_name: stringify!($token) },)*
            $(NativeSpec { native: CoreNative::$named_token, opcode: None, token_name: stringify!($named_token) },)*
        ];

        const _: () = {
            let opcodes: &[i32] = &[$($opcode),*];
            let mut i = 0;
            while i < opcodes.len() {
                assert!(opcodes[i] >= 0 && (opcodes[i] as usize) < NATIVE_COUNT);
                let mut j = 0;
                while j < i { assert!(opcodes[i] != opcodes[j]); j += 1; }
                i += 1;
            }
        };
    };
}

/// Expands the one Core identity list for the typed catalog and legacy adapter.
#[macro_export]
macro_rules! core_native_registration_list {
    ($consumer:ident) => {
        $consumer! {
            checked {
            (native_opcode!(EX_Nothing), intUObjectexecNothing),
            (native_opcode!(EX_EndFunctionParms), intUObjectexecEndFunctionParms),
            (native_opcode!(EX_Stop), intUObjectexecStop),
            (native_opcode!(EX_IntConst), intUObjectexecIntConst),
            (native_opcode!(EX_FloatConst), intUObjectexecFloatConst),
            (native_opcode!(EX_ObjectConst), intUObjectexecObjectConst),
            (native_opcode!(EX_StringConst), intUObjectexecStringConst),
            (native_opcode!(EX_UnicodeStringConst), intUObjectexecUnicodeStringConst),
            (native_opcode!(EX_NameConst), intUObjectexecNameConst),
            (native_opcode!(EX_ByteConst), intUObjectexecByteConst),
            (native_opcode!(EX_IntZero), intUObjectexecIntZero),
            (native_opcode!(EX_IntOne), intUObjectexecIntOne),
            (native_opcode!(EX_True), intUObjectexecTrue),
            (native_opcode!(EX_False), intUObjectexecFalse),
            (native_opcode!(EX_NoObject), intUObjectexecNoObject),
            (native_opcode!(EX_IntConstByte), intUObjectexecIntConstByte),
            (native_opcode!(EX_ByteToInt), intUObjectexecByteToInt),
            (native_opcode!(EX_ByteToBool), intUObjectexecByteToBool),
            (native_opcode!(EX_ByteToFloat), intUObjectexecByteToFloat),
            (native_opcode!(EX_IntToByte), intUObjectexecIntToByte),
            (native_opcode!(EX_IntToBool), intUObjectexecIntToBool),
            (native_opcode!(EX_IntToFloat), intUObjectexecIntToFloat),
            (native_opcode!(EX_BoolToByte), intUObjectexecBoolToByte),
            (native_opcode!(EX_BoolToInt), intUObjectexecBoolToInt),
            (native_opcode!(EX_BoolToFloat), intUObjectexecBoolToFloat),
            (native_opcode!(EX_FloatToByte), intUObjectexecFloatToByte),
            (native_opcode!(EX_FloatToInt), intUObjectexecFloatToInt),
            (native_opcode!(EX_FloatToBool), intUObjectexecFloatToBool),
            (native_opcode!(EX_ObjectToBool), intUObjectexecObjectToBool),
            (native_opcode!(EX_NameToBool), intUObjectexecNameToBool),
            (129, intUObjectexecNot_PreBool),
            (242, intUObjectexecEqualEqual_BoolBool),
            (243, intUObjectexecNotEqual_BoolBool),
            (131, intUObjectexecXorXor_BoolBool),
            (141, intUObjectexecComplement_PreInt),
            (196, intUObjectexecGreaterGreaterGreater_IntInt),
            (143, intUObjectexecSubtract_PreInt),
            (144, intUObjectexecMultiply_IntInt),
            (145, intUObjectexecDivide_IntInt),
            (146, intUObjectexecAdd_IntInt),
            (147, intUObjectexecSubtract_IntInt),
            (148, intUObjectexecLessLess_IntInt),
            (149, intUObjectexecGreaterGreater_IntInt),
            (150, intUObjectexecLess_IntInt),
            (151, intUObjectexecGreater_IntInt),
            (152, intUObjectexecLessEqual_IntInt),
            (153, intUObjectexecGreaterEqual_IntInt),
            (154, intUObjectexecEqualEqual_IntInt),
            (155, intUObjectexecNotEqual_IntInt),
            (156, intUObjectexecAnd_IntInt),
            (157, intUObjectexecXor_IntInt),
            (158, intUObjectexecOr_IntInt),
            (249, intUObjectexecMin),
            (250, intUObjectexecMax),
            (251, intUObjectexecClamp),
            (169, intUObjectexecSubtract_PreFloat),
            (170, intUObjectexecMultiplyMultiply_FloatFloat),
            (171, intUObjectexecMultiply_FloatFloat),
            (172, intUObjectexecDivide_FloatFloat),
            (173, intUObjectexecPercent_FloatFloat),
            (174, intUObjectexecAdd_FloatFloat),
            (175, intUObjectexecSubtract_FloatFloat),
            (176, intUObjectexecLess_FloatFloat),
            (177, intUObjectexecGreater_FloatFloat),
            (178, intUObjectexecLessEqual_FloatFloat),
            (179, intUObjectexecGreaterEqual_FloatFloat),
            (180, intUObjectexecEqualEqual_FloatFloat),
            (181, intUObjectexecNotEqual_FloatFloat),
            (210, intUObjectexecComplementEqual_FloatFloat),
            (186, intUObjectexecAbs),
            (187, intUObjectexecSin),
            (188, intUObjectexecCos),
            (189, intUObjectexecTan),
            (190, intUObjectexecAtan),
            (191, intUObjectexecExp),
            (192, intUObjectexecLoge),
            (193, intUObjectexecSqrt),
            (194, intUObjectexecSquare),
            (244, intUObjectexecFMin),
            (245, intUObjectexecFMax),
            (246, intUObjectexecFClamp),
            (247, intUObjectexecLerp),
            (248, intUObjectexecSmerp),
            (34, intUObjectexecRotationConst),
            (35, intUObjectexecVectorConst),
            (native_opcode!(EX_VectorToBool), intUObjectexecVectorToBool),
            (native_opcode!(EX_VectorToRotator), intUObjectexecVectorToRotator),
            (native_opcode!(EX_RotatorToBool), intUObjectexecRotatorToBool),
            (211, intUObjectexecSubtract_PreVector),
            (212, intUObjectexecMultiply_VectorFloat),
            (213, intUObjectexecMultiply_FloatVector),
            (296, intUObjectexecMultiply_VectorVector),
            (214, intUObjectexecDivide_VectorFloat),
            (215, intUObjectexecAdd_VectorVector),
            (216, intUObjectexecSubtract_VectorVector),
            (217, intUObjectexecEqualEqual_VectorVector),
            (218, intUObjectexecNotEqual_VectorVector),
            (219, intUObjectexecDot_VectorVector),
            (220, intUObjectexecCross_VectorVector),
            (225, intUObjectexecVSize),
            (226, intUObjectexecNormal),
            (300, intUObjectexecMirrorVectorByNormal),
            (142, intUObjectexecEqualEqual_RotatorRotator),
            (203, intUObjectexecNotEqual_RotatorRotator),
            (287, intUObjectexecMultiply_RotatorFloat),
            (288, intUObjectexecMultiply_FloatRotator),
            (289, intUObjectexecDivide_RotatorFloat),
            (316, intUObjectexecAdd_RotatorRotator),
            (317, intUObjectexecSubtract_RotatorRotator),
            (229, intUObjectexecGetAxes),
            (230, intUObjectexecGetUnAxes),
            (1031, intUObjectexecNormalize),
            (275, intUObjectexecLessLess_VectorRotator),
            (276, intUObjectexecGreaterGreater_VectorRotator),
            (1030, intUObjectexecOrthoRotation),
            (227, intUObjectexecInvert),
            (96, intUObjectexecHighNative0),
            (97, intUObjectexecHighNative1),
            (98, intUObjectexecHighNative2),
            (99, intUObjectexecHighNative3),
            (100, intUObjectexecHighNative4),
            (101, intUObjectexecHighNative5),
            (102, intUObjectexecHighNative6),
            (103, intUObjectexecHighNative7),
            (104, intUObjectexecHighNative8),
            (105, intUObjectexecHighNative9),
            (106, intUObjectexecHighNative10),
            (107, intUObjectexecHighNative11),
            (108, intUObjectexecHighNative12),
            (109, intUObjectexecHighNative13),
            (110, intUObjectexecHighNative14),
            (111, intUObjectexecHighNative15),
            (native_opcode!(EX_EatString), intUObjectexecEatString),
            (112, intUObjectexecConcat_StringString),
            (168, intUObjectexecAt_StringString),
            (115, intUObjectexecLess_StringString),
            (116, intUObjectexecGreater_StringString),
            (120, intUObjectexecLessEqual_StringString),
            (121, intUObjectexecGreaterEqual_StringString),
            (122, intUObjectexecEqualEqual_StringString),
            (123, intUObjectexecNotEqual_StringString),
            (124, intUObjectexecComplementEqual_StringString),
            (125, intUObjectexecLen),
            (126, intUObjectexecInStr),
            (127, intUObjectexecMid),
            (128, intUObjectexecLeft),
            (234, intUObjectexecRight),
            (235, intUObjectexecCaps),
            (236, intUObjectexecChr),
            (237, intUObjectexecAsc),
            (254, intUObjectexecEqualEqual_NameName),
            (255, intUObjectexecNotEqual_NameName),
            (114, intUObjectexecEqualEqual_ObjectObject),
            (119, intUObjectexecNotEqual_ObjectObject),
            (native_opcode!(EX_ByteToString), intUObjectexecByteToString),
            (native_opcode!(EX_IntToString), intUObjectexecIntToString),
            (native_opcode!(EX_BoolToString), intUObjectexecBoolToString),
            (native_opcode!(EX_FloatToString), intUObjectexecFloatToString),
            (native_opcode!(EX_StringToByte), intUObjectexecStringToByte),
            (native_opcode!(EX_StringToInt), intUObjectexecStringToInt),
            (native_opcode!(EX_StringToBool), intUObjectexecStringToBool),
            (native_opcode!(EX_StringToFloat), intUObjectexecStringToFloat),
            (native_opcode!(EX_ObjectToString), intUObjectexecObjectToString),
            (native_opcode!(EX_NameToString), intUObjectexecNameToString),
            (native_opcode!(EX_StringToName), intUObjectexecStringToName),
            (native_opcode!(EX_StringToVector), intUObjectexecStringToVector),
            (native_opcode!(EX_StringToRotator), intUObjectexecStringToRotator),
            (native_opcode!(EX_VectorToString), intUObjectexecVectorToString),
            (native_opcode!(EX_RotatorToVector), intUObjectexecRotatorToVector),
            (native_opcode!(EX_RotatorToString), intUObjectexecRotatorToString),
            (130, intUObjectexecAndAnd_BoolBool),
            (132, intUObjectexecOrOr_BoolBool),
            (133, intUObjectexecMultiplyEqual_ByteByte),
            (134, intUObjectexecDivideEqual_ByteByte),
            (135, intUObjectexecAddEqual_ByteByte),
            (136, intUObjectexecSubtractEqual_ByteByte),
            (137, intUObjectexecAddAdd_PreByte),
            (138, intUObjectexecSubtractSubtract_PreByte),
            (139, intUObjectexecAddAdd_Byte),
            (140, intUObjectexecSubtractSubtract_Byte),
            (159, intUObjectexecMultiplyEqual_IntFloat),
            (160, intUObjectexecDivideEqual_IntFloat),
            (161, intUObjectexecAddEqual_IntInt),
            (162, intUObjectexecSubtractEqual_IntInt),
            (163, intUObjectexecAddAdd_PreInt),
            (164, intUObjectexecSubtractSubtract_PreInt),
            (165, intUObjectexecAddAdd_Int),
            (166, intUObjectexecSubtractSubtract_Int),
            (182, intUObjectexecMultiplyEqual_FloatFloat),
            (183, intUObjectexecDivideEqual_FloatFloat),
            (184, intUObjectexecAddEqual_FloatFloat),
            (185, intUObjectexecSubtractEqual_FloatFloat),
            (221, intUObjectexecMultiplyEqual_VectorFloat),
            (297, intUObjectexecMultiplyEqual_VectorVector),
            (222, intUObjectexecDivideEqual_VectorFloat),
            (223, intUObjectexecAddEqual_VectorVector),
            (224, intUObjectexecSubtractEqual_VectorVector),
            (290, intUObjectexecMultiplyEqual_RotatorFloat),
            (291, intUObjectexecDivideEqual_RotatorFloat),
            (318, intUObjectexecAddEqual_RotatorRotator),
            (319, intUObjectexecSubtractEqual_RotatorRotator),
            (native_opcode!(EX_DynamicCast), intUObjectexecDynamicCast),
            (native_opcode!(EX_MetaCast), intUObjectexecMetaCast),
            (258, intUObjectexecClassIsChildOf),
            (303, intUObjectexecIsA),
            (167, intUObjectexecRand),
            (195, intUObjectexecFRand),
            (252, intUObjectexecVRand),
            (320, intUObjectexecRotRand),
            (1033, intUObjectexecRandRange),
            (native_opcode!(EX_New), intUObjectexecNew),
            (113, intUObjectexecGotoState),
            (117, intUObjectexecEnable),
            (118, intUObjectexecDisable),
            (281, intUObjectexecIsInState),
            (284, intUObjectexecGetStateName),
            (231, intUObjectexecLog),
            (232, intUObjectexecWarn),
            (536, intUObjectexecSaveConfig),
            (543, intUObjectexecResetConfig),
            (native_opcode!(EX_LocalVariable), intUObjectexecLocalVariable),
            (native_opcode!(EX_InstanceVariable), intUObjectexecInstanceVariable),
            (native_opcode!(EX_DefaultVariable), intUObjectexecDefaultVariable),
            (native_opcode!(EX_ClassContext), intUObjectexecClassContext),
            (native_opcode!(EX_ArrayElement), intUObjectexecArrayElement),
            (native_opcode!(EX_DynArrayElement), intUObjectexecDynArrayElement),
            (native_opcode!(EX_DynArrayCount), intUObjectexecDynArrayCount),
            (native_opcode!(EX_BoolVariable), intUObjectexecBoolVariable),
            (native_opcode!(EX_StructMember), intUObjectexecStructMember),
            (native_opcode!(EX_NativeParm), intUObjectexecNativeParm),
            (native_opcode!(EX_Let), intUObjectexecLet),
            (native_opcode!(EX_LetBool), intUObjectexecLetBool),
            (native_opcode!(EX_Self), intUObjectexecSelf),
            (native_opcode!(EX_Context), intUObjectexecContext),
            (native_opcode!(EX_StructCmpEq), intUObjectexecStructCmpEq),
            (native_opcode!(EX_StructCmpNe), intUObjectexecStructCmpNe),
            (native_opcode!(EX_DebugInfo), intUObjectexecDebugInfo),
            (native_opcode!(EX_Switch), intUObjectexecSwitch),
            (native_opcode!(EX_Case), intUObjectexecCase),
            (native_opcode!(EX_Jump), intUObjectexecJump),
            (native_opcode!(EX_JumpIfNot), intUObjectexecJumpIfNot),
            (native_opcode!(EX_Assert), intUObjectexecAssert),
            (native_opcode!(EX_GotoLabel), intUObjectexecGotoLabel),
            (native_opcode!(EX_Iterator), intUObjectexecIterator),
            (native_opcode!(EX_VirtualFunction), intUObjectexecVirtualFunction),
            (native_opcode!(EX_FinalFunction), intUObjectexecFinalFunction),
            (native_opcode!(EX_GlobalFunction), intUObjectexecGlobalFunction),
            }
            named {
            (intUCommandletexecMain),
            (intUObjectexecLocalize),
            (intUObjectexecGetPropertyText),
            (intUObjectexecSetPropertyText),
            (intUObjectexecStaticSaveConfig),
            (intUObjectexecGetEnum),
            (intUObjectexecGetLanguage),
            (intUObjectexecDynamicLoadObject),
            }
        }
    };
}

core_native_registration_list!(define_catalog);
