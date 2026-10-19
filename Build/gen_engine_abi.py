#!/usr/bin/env python3
"""Generate the hp2-abi engine layout mirrors and the C++ static_assert TU.

The MIRRORS manifest names the C++ members each Rust mirror must reproduce.
The generator compiles a probe with the exact flags of `Engine/Src/UnWind.cpp`
from `out/macos-arm64/compile_commands.json`, runs it, and reads the real
`__builtin_offsetof` / `sizeof` results. Bitfield bit positions come from a
runtime bit scan of a zeroed object. Single-inheritance bases are embedded as
the first member (`base`), with the base offset read from the probe. Gaps are
filled with byte arrays, so the Rust layout is byte-identical to C++.

Outputs (checked in; regenerate with `python3 Build/gen_engine_abi.py`):
  crates/hp2-abi/src/engine/generated.rs
  HarryPotter2/Unreal/Engine/Src/HP2EngineAbiAsserts.cpp

Mirrors are partial views: only the listed members are named. Everything else
in the C++ object is opaque padding, so a mirror is never larger or smaller
than its C++ type.
"""

import argparse
import json
import re
import shlex
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UNREAL = ROOT / "HarryPotter2" / "Unreal"
COMPILE_COMMANDS = ROOT / "out" / "macos-arm64" / "compile_commands.json"
PROBE_FLAG_SOURCE = "Engine/Src/UnWind.cpp"
RUST_OUT = ROOT / "crates" / "hp2-abi" / "src" / "engine" / "generated.rs"
CPP_OUT = UNREAL / "Engine" / "Src" / "HP2EngineAbiAsserts.cpp"

# Manifest key -> size in bytes. `FVector` and `FRotator` are struct mirrors;
# `FVector[16]` is a fixed array of FVector.
SCALAR_SIZES = {
    "f32": 4,
    "i32": 4,
    "u32": 4,
    "i16": 2,
    "u8": 1,
    "FVector": 12,
    "FVector[16]": 192,
    "FRotator": 12,
    "FPlane": 16,
    "ptr": 8,
    "ptr:ANavigationPoint": 8,
    "i32[16]": 64,
    "ptr[16]": 128,
    "ptr[4]": 32,
    "TArray": 16,
    "TRefArray": 16,
    "TTransArray": 24,
    "FName": 4,
    "f32[8]": 32,
    "FVector[8]": 96,
    "FRotator[8]": 96,
    "FPointRegion": 16,
    "ptr[8]": 64,
    "FName[8]": 32,
    "u16": 2,
    "u16[3]": 6,
    "f32[4]": 16,
    "FPlace": 28,
    "VJointPos": 44,
    "FBox": 28,
    "u8[255]": 255,
    "FCoords": 48,
    "FSoundData": 48,
    "FLipSyncData": 56,
    "FFloatParams": 8,
    "FColorParams": 8,
    "u8[4]": 4,
}
RUST_TYPE = {
    "ptr": "*mut c_void",
    "ptr:ANavigationPoint": "*mut ANavigationPoint",
    "FVector[16]": "[FVector; 16]",
    "i32[16]": "[i32; 16]",
    "ptr[16]": "[*mut c_void; 16]",
    "ptr[4]": "[*mut c_void; 4]",
    "TArray": "TArrayRaw",
    "TRefArray": "TRefArrayRaw",
    "TTransArray": "[u8; 24]",
    "u32": "u32",
    "i16": "i16",
    "FName": "[u8; 4]",
    "f32[8]": "[f32; 8]",
    "FVector[8]": "[FVector; 8]",
    "FName[8]": "[[u8; 4]; 8]",
    "ptr[8]": "[*mut c_void; 8]",
    "FRotator[8]": "[FRotator; 8]",
    "FPlane": "FPlane",
    "u16": "u16",
    "u16[3]": "[u16; 3]",
    "f32[4]": "[f32; 4]",
    "FPlace": "FPlace",
    "VJointPos": "VJointPos",
    "FBox": "FBox",
    "FCoords": "FCoords",
    "u8[255]": "[u8; 255]",
    "FFloatParams": "FFloatParams",
    "FColorParams": "FColorParams",
    "u8[4]": "[u8; 4]",
}


@dataclass
class Mirror:
    rust: str  # Rust struct name in hp2_abi::engine::generated
    cpp: str  # C++ type whose layout is mirrored
    fields: list  # (C++ member, manifest key)
    bits: list = field(default_factory=list)  # 1-bit BITFIELD members
    base: str = None  # C++ base mirrored as the first member (Rust name = C++ name)
    packed: bool = False  # C++ class sits inside `#pragma pack(4)` (EngineClasses.h); base mirrors inherit it
    private: tuple = ()  # C++ members private to the class; probe sees them (-fno-access-control), C++ static_asserts skip them


MIRRORS = [
    Mirror("FRotator", "FRotator", [("Pitch", "i32"), ("Yaw", "i32"), ("Roll", "i32")]),
    Mirror(
        "AActor",
        "AActor",
        [
            ("XLevel", "ptr"),
            ("Physics", "u8"),
            ("Location", "FVector"),
            ("Rotation", "FRotator"),
            ("CollisionRadius", "f32"),
            ("CollisionWidth", "f32"),
            ("CollisionHeight", "f32"),
            ("Brush", "ptr"),
            ("CollisionTag", "i32"),
            ("ColLocation", "FVector"),
            ("LatentFloat", "f32"),
            ("Velocity", "FVector"),
            ("IPSpeed", "f32"),
            ("Acceleration", "FVector"),
            ("Region", "FPointRegion"),
            ("Mass", "f32"),
            ("Buoyancy", "f32"),
            ("Instigator", "ptr"),
            ("OldLocation", "FVector"),
            ("DesiredRotation", "FRotator"),
            ("NetPriority", "f32"),
            ("Base", "ptr"),
            ("RotationRate", "FRotator"),
            ("PendingTouch", "ptr"),
            ("SavedPrePivotZ", "f32"),
            ("Owner", "ptr"),
            ("Tag", "FName"),
            ("StandingCount", "u8"),
            ("Touching", "ptr[4]"),
            ("Role", "u8"),
            ("SkelAnim", "ptr"),
            ("AnimSequence", "FName"),
            ("AnimFrame", "f32"),
            ("AnimLast", "f32"),
            ("TweenAlpha", "f32"),
            ("TweenRate", "f32"),
            ("AnimBone", "u8"),
            ("Mesh", "ptr"),
            ("DrawScale", "f32"),
            ("PrePivot", "FVector"),
            ("Wideness", "u8"),
            ("CollideType", "u8"),
            ("AuxAnims", "TArray"),
            ("Level", "ptr"),
            ("DrawType", "u8"),
        ],
        ["bDeleteMe", "bCollideActors", "bIsPawn", "bJustTeleported", "bNetOwner", "bHidden", "bBlockActors", "bFixedRotationDir", "bRotateToDesired", "bStatic", "bMovable", "bCollideWorld", "bBlockPlayers", "bCollideWhenPlacing", "bBounce", "bInterpolating", "bInterpolating_IgnoreRot", "bAlignBottom", "bAnimByOwner", "bAnimMove", "bAnimTransient", "bAnimLoop"],
        packed=True,
    ),
    Mirror(
        "APawn",
        "APawn",
        [
            ("GroundSpeed", "f32"),
            ("JumpZ", "f32"),
            ("MaxStepHeight", "f32"),
            ("BaseEyeHeight", "f32"),
            ("MoveTarget", "ptr"),
            ("RouteCache", "ptr[16]"),
            ("Destination", "FVector"),
            ("AccelRate", "f32"),
            ("AirControl", "f32"),
            ("DesiredSpeed", "f32"),
            ("MaxMountHeight", "f32"),
            ("MinHitWall", "f32"),
            ("FootRegion", "FPointRegion"),
            ("HeadRegion", "FPointRegion"),
            ("MeleeRange", "f32"),
            ("ReducedDamageType", "FName"),
            ("Enemy", "ptr"),
            ("Weapon", "ptr"),
            ("SpecialGoal", "ptr"),
            ("nextPawn", "ptr"),
            ("noise1other", "ptr"),
            ("noise2other", "ptr"),
            ("PlayerReplicationInfo", "ptr"),
            ("Visibility", "u8"),
            ("Alertness", "f32"),
            ("Stimulus", "f32"),
            ("SightRadius", "f32"),
            ("PeripheralVision", "f32"),
            ("HearingThreshold", "f32"),
            ("Skill", "f32"),
            ("LastSeenPos", "FVector"),
            ("LastSeeingPos", "FVector"),
            ("LastSeenTime", "f32"),
            ("noise1spot", "FVector"),
            ("noise1time", "f32"),
            ("noise1loudness", "f32"),
            ("noise2spot", "FVector"),
            ("noise2time", "f32"),
            ("noise2loudness", "f32"),
            ("MoveTimer", "f32"),
            ("AirSpeed", "f32"),
            ("WaterSpeed", "f32"),
            ("AvgPhysicsTime", "f32"),
            ("StepHeight", "f32"),
            ("StepWidth", "f32"),
            ("ViewRotation", "FRotator"),
            ("EyeHeight", "f32"),
            ("FaceTarget", "ptr"),
            ("Focus", "FVector"),
            ("MaxDesiredSpeed", "f32"),
            ("Floor", "FVector"),
            ("SpecialPause", "f32"),
            ("SightCounter", "f32"),
        ],
        ["bCanJump", "bCanWalk", "bCanSwim", "bCanFly", "bIsPlayer", "bCanOpenDoors", "bCanDoSpecial", "bHunting", "bIsWalking", "bFromWall", "bLOSflag", "bReducedSpeed", "bCanStrafe", "bJumpOffPawn", "bHitSlopedWall", "bAvoidLedges", "bNoZoneFriction", "bStopAtLedges", "bShootSpecial", "bAdvancedTactics"],
        base="AActor",
    ),
    Mirror(
        "APlayerPawn",
        "APlayerPawn",
        [
            ("aBaseX", "f32"),
            ("aForward", "f32"),
            ("aTurn", "f32"),
            ("aStrafe", "f32"),
            ("aLookUp", "f32"),
            ("aSideMove", "f32"),
            ("SmoothMouseX", "f32"),
            ("SmoothMouseY", "f32"),
            ("DesiredFOV", "f32"),
            ("ViewTarget", "ptr"),
        ],
        ["bAutoJump", "bShowMenu", "bIsCaptured", "bModernThirdPersonControls"],
        base="APawn",
    ),
    Mirror(
        "AWind",
        "AWind",
        [
            ("WindSpeed", "f32"),
            ("WindRadius", "u8"),
            ("WindRadiusInner", "u8"),
            ("WindFluctuation", "u8"),
            ("WindFlucPeriod", "u8"),
            ("WindSource", "u8"),
            ("Fluc", "FVector"),
            ("FlucVel", "FVector"),
        ],
        ["bPermeating"],
        base="AActor",
    ),
    Mirror("ULevel", "ULevel", [("Actors", "TTransArray"), ("NumMoves", "i32"), ("MoveCycles", "i32"), ("Model", "ptr"), ("ReachSpecs", "TArray"), ("Hash", "ptr"), ("BrushTracker", "ptr")]),
    Mirror(
        "ANavigationPoint",
        "ANavigationPoint",
        [
            ("Paths", "i32[16]"),
            ("upstreamPaths", "i32[16]"),
            ("PrunedPaths", "i32[16]"),
            ("visitedWeight", "i32"),
            ("bestPathWeight", "i32"),
            ("nextNavigationPoint", "ptr:ANavigationPoint"),
            ("nextOrdered", "ptr:ANavigationPoint"),
            ("prevOrdered", "ptr:ANavigationPoint"),
            ("startPath", "ptr"),
            ("previousPath", "ptr"),
            ("cost", "i32"),
            ("ExtraCost", "i32"),
        ],
        ["bPlayerOnly", "bEndPoint", "bSpecialCost"],
        base="AActor",
    ),
    Mirror(
        "AInventory",
        "AInventory",
        [("MaxDesireability", "f32"), ("MyMarker", "ptr")],
        base="AActor",
    ),
    Mirror(
        "AInventorySpot",
        "AInventorySpot",
        [("markedItem", "ptr")],
        base="ANavigationPoint",
    ),
    Mirror(
        "FReachSpec",
        "FReachSpec",
        [
            ("distance", "i32"),
            ("Start", "ptr"),
            ("End", "ptr"),
            ("CollisionRadius", "i32"),
            ("CollisionHeight", "i32"),
            ("reachFlags", "i32"),
            ("bPruned", "u8"),
        ],
    ),
    Mirror(
        "FPoly",
        "FPoly",
        [
            ("Base", "FVector"),
            ("Normal", "FVector"),
            ("TextureU", "FVector"),
            ("TextureV", "FVector"),
            ("Vertex", "FVector[16]"),
            ("PolyFlags", "u32"),
            ("Actor", "ptr"),
            ("Texture", "ptr"),
            ("NumVertices", "i32"),
            ("iLink", "i32"),
            ("iBrushPoly", "i32"),
            ("PanU", "i16"),
            ("PanV", "i16"),
            ("SavePolyIndex", "i32"),
            ("bFaceDragSel", "i32"),
        ],
    ),
    Mirror(
        "FBox",
        "FBox",
        [("Min", "FVector"), ("Max", "FVector"), ("IsValid", "u8")],
    ),
    Mirror(
        "FCheckResult",
        "FCheckResult",
        [
            ("Actor", "ptr"),
            ("Location", "FVector"),
            ("Normal", "FVector"),
            ("Primitive", "ptr"),
            ("Time", "f32"),
            ("Item", "i32"),
            ("Next", "ptr"),
        ],
    ),
    Mirror(
        "AMover",
        "AMover",
        [
            ("MoverEncroachType", "u8"),
            ("MoverGlideType", "u8"),
            ("BumpType", "u8"),
            ("KeyNum", "u8"),
            ("PrevKeyNum", "u8"),
            ("NumKeys", "u8"),
            ("WorldRaytraceKey", "u8"),
            ("BrushRaytraceKey", "u8"),
            ("MoveTimes", "f32[8]"),
            ("MoveTime", "f32"),
            ("StayOpenTime", "f32"),
            ("OtherTime", "f32"),
            ("EncroachDamage", "i32"),
            ("PlayerBumpEvent", "FName"),
            ("BumpEvent", "FName"),
            ("SavedTrigger", "ptr"),
            ("DamageThreshold", "f32"),
            ("numTriggerEvents", "i32"),
            ("Leader", "ptr"),
            ("Follower", "ptr"),
            ("ReturnGroup", "FName"),
            ("DelayTime", "f32"),
            ("AttachTag", "FName"),
            ("OpeningSound", "ptr"),
            ("OpenedSound", "ptr"),
            ("ClosingSound", "ptr"),
            ("ClosedSound", "ptr"),
            ("MoveAmbientSound", "ptr"),
            ("FailSound", "ptr"),
            ("MoverRadius", "f32"),
            ("MoverVolume", "u8"),
            ("MoverPitch", "u8"),
            ("KeyPos", "FVector[8]"),
            ("KeyRot", "FRotator[8]"),
            ("BasePos", "FVector"),
            ("OldPos", "FVector"),
            ("OldPrePivot", "FVector"),
            ("SavedPos", "FVector"),
            ("BaseRot", "FRotator"),
            ("OldRot", "FRotator"),
            ("SavedRot", "FRotator"),
            ("PhysAlpha", "f32"),
            ("PhysRate", "f32"),
            ("MyMarker", "ptr"),
            ("TriggerActor", "ptr"),
            ("TriggerActor2", "ptr"),
            ("WaitingPawn", "ptr"),
            ("RecommendedTrigger", "ptr"),
            ("SimOldPos", "FVector"),
            ("SimOldRotPitch", "i32"),
            ("SimOldRotYaw", "i32"),
            ("SimOldRotRoll", "i32"),
            ("SimInterpolate", "FVector"),
            ("RealPosition", "FVector"),
            ("RealRotation", "FRotator"),
            ("ClientUpdate", "i32"),
            ("MoverSpringTime", "f32"),
            ("MoverMaxAmplitude", "f32"),
            ("MoverFluctuations", "u8"),
            ("HitPosition", "FVector"),
            ("HitNormal", "FVector"),
        ],
        base="AActor",
    ),
    Mirror("FPointRegion", "FPointRegion", [("Zone", "ptr"), ("iLeaf", "i32"), ("ZoneNumber", "u8")]),
    Mirror("AInfo", "AInfo", [], base="AActor", packed=True),
    Mirror("AKeypoint", "AKeypoint", [], base="AActor", packed=True),
    Mirror(
        "UParticle",
        "UParticle",
        [
            ("LastPosition", "FVector"),
            ("Position", "FVector"),
            ("Velocity", "FVector"),
            ("Age", "f32"),
            ("Lifetime", "f32"),
            ("Alpha", "f32"),
            ("AlphaFull", "f32"),
            ("AlphaDelta", "f32"),
            ("Color", "FPlane"),
            ("ColorDelta", "FPlane"),
            ("Width", "f32"),
            ("WidthFull", "f32"),
            ("WidthDelta", "f32"),
            ("Length", "f32"),
            ("LengthFull", "f32"),
            ("LengthDelta", "f32"),
            ("Tail", "f32"),
            ("DripTimer", "f32"),
            ("ChaosDelay", "f32"),
            ("Spin", "f32"),
            ("SpinRate", "f32"),
            ("Id", "i32"),
            ("PriorityTag", "i32"),
        ],
    ),
    Mirror("FFloatParams", "FFloatParams", [("Base", "f32"), ("Rand", "f32")]),
    Mirror("FColorParams", "FColorParams", [("Base", "u8[4]"), ("Rand", "u8[4]")]),
    Mirror(
        "AParticleFX",
        "AParticleFX",
        [
            ("ParticlesPerSec", "FFloatParams"),
            ("SourceWidth", "FFloatParams"),
            ("SourceHeight", "FFloatParams"),
            ("SourceDepth", "FFloatParams"),
            ("Period", "FFloatParams"),
            ("Decay", "FFloatParams"),
            ("AngularSpreadWidth", "FFloatParams"),
            ("AngularSpreadHeight", "FFloatParams"),
            ("Speed", "FFloatParams"),
            ("Lifetime", "FFloatParams"),
            ("AlphaStart", "FFloatParams"),
            ("AlphaEnd", "FFloatParams"),
            ("SizeWidth", "FFloatParams"),
            ("SizeLength", "FFloatParams"),
            ("SizeEndScale", "FFloatParams"),
            ("SpinRate", "FFloatParams"),
            ("DripTime", "FFloatParams"),
            ("ColorStart", "FColorParams"),
            ("ColorEnd", "FColorParams"),
            ("Attraction", "FVector"),
            ("Gravity", "FVector"),
            ("Damping", "f32"),
            ("WindModifier", "f32"),
            ("GravityModifier", "f32"),
            ("ParentBlend", "f32"),
            ("LOD", "f32"),
            ("Chaos", "f32"),
            ("ChaosDelay", "f32"),
            ("Elasticity", "f32"),
            ("AlphaDelay", "f32"),
            ("ColorDelay", "f32"),
            ("SizeDelay", "f32"),
            ("AlphaGrowPeriod", "f32"),
            ("SizeGrowPeriod", "f32"),
            ("EmitDelay", "f32"),
            ("EmissionResidue", "f32"),
            ("Age", "f32"),
            ("ElapsedTime", "f32"),
            ("LastEmitLocation", "FVector"),
            ("ParticlesAlive", "i32"),
            ("ParticlesMax", "i32"),
            ("ParticlesEmitted", "i32"),
            ("CurrentPriorityTag", "i32"),
            ("ColorPalette", "ptr"),
            ("Pattern", "ptr"),
            ("ParticleList", "ptr"),
            ("Distribution", "u8"),
            ("RenderPrimitive", "u8"),
            ("LastUpdateLocation", "FVector"),
            ("LastUpdateRotation", "FRotator"),
        ],
        ["bSteadyState", "bPrime", "bUpdate", "bSystemRelative", "bVelocityRelative", "bWindPerParticle", "bEmit", "bShellOnly"],
        base="AActor",
        packed=True,
    ),
    Mirror(
        "AInterpolationPoint",
        "AInterpolationPoint",
        [
            ("Pause", "f32[8]"),
            ("ViewTargetTag", "FName[8]"),
            ("ViewTarget", "ptr[8]"),
            ("Prev", "ptr"),
            ("Next", "ptr"),
            ("StartControlPoint", "FVector"),
            ("EndControlPoint", "FVector"),
            ("Smoothing", "f32"),
            ("DesiredSpeed", "f32"),
            ("PathDist", "f32"),
        ],
        ["bEndOfPath", "bInstantNextPath", "bFaceMoveDirection", "bConstantSpeed", "bNewRotationSmoothing"],
        base="AKeypoint",
        packed=True,
    ),
    Mirror(
        "AInterpolationManager",
        "AInterpolationManager",
        [
            ("Dest", "ptr"),
            ("Last", "ptr"),
            ("PhysAlpha", "f32"),
            ("PhysRate", "f32"),
            ("RemainingPause", "f32"),
            ("StartPause", "f32"),
            ("PauseNum", "i32"),
            ("TurnRateX", "FVector"),
            ("TurnRateZ", "FVector"),
            ("OldDesiredX", "FVector"),
            ("OldDesiredZ", "FVector"),
        ],
        ["bInstantMove"],
        base="AInfo",
        packed=True,
    ),
    Mirror(
        "AZoneInfo",
        "AZoneInfo",
        [("ZoneGravity", "FVector"), ("ZoneVelocity", "FVector"), ("ZoneFluidFriction", "f32"), ("ZoneTerminalVelocity", "f32"), ("ZoneGroundFriction", "f32"), ("DamageType", "FName")],
        ["bWaterZone", "bBounceVelocity", "bPainZone"],
        base="AInfo",
        packed=True,
    ),
    Mirror(
        "AWarpZoneMarker",
        "AWarpZoneMarker",
        [("markedWarpZone", "ptr")],
        [],
        base="ANavigationPoint",
    ),
    Mirror(
        "ADecoration",
        "ADecoration",
        [("numLandings", "i32")],
        ["bPushable", "bBobbing"],
        base="AActor",
        packed=True,
    ),
    Mirror(
        "ACarcass",
        "ACarcass",
        [],
        ["bSlidingCarcass"],
        base="ADecoration",
        packed=True,
    ),
    Mirror("FPlace", "FPlace", [("Quat", "f32[4]"), ("Pos", "FVector")]),
    Mirror("VJointPos", "VJointPos", [("Place", "FPlace"), ("Length", "f32"), ("XSize", "f32"), ("YSize", "f32"), ("ZSize", "f32")]),
    Mirror("FMeshBone", "FMeshBone", [("Name", "FName"), ("Flags", "u32"), ("BonePos", "VJointPos"), ("ParentIndex", "i32"), ("NumChildren", "i32"), ("Depth", "i32")]),
    Mirror("FNamedBone", "FNamedBone", [("Name", "FName"), ("Flags", "u32"), ("ParentIndex", "i32")]),
    Mirror("VBoneInfIndex", "VBoneInfIndex", [("WeightIndex", "u16"), ("Number", "u16"), ("DetailA", "u16"), ("DetailB", "u16")]),
    Mirror("VBoneInfluence", "VBoneInfluence", [("PointIndex", "u16"), ("BoneWeight", "u16")]),
    Mirror("FAnalogTrack", "FAnalogTrack", [("Flags", "u32"), ("KeyQuat", "TRefArray"), ("KeyPos", "TRefArray"), ("KeyDelta", "TRefArray"), ("PosScale", "f32"), ("TimeScale", "f32")]),
    Mirror("MotionChunk", "MotionChunk", [("RootSpeed3D", "FVector"), ("TrackTime", "f32"), ("StartBone", "i32"), ("Flags", "u32"), ("BoneIndices", "TArray"), ("AnimTracks", "TArray")]),
    Mirror("FMeshAnimSeq", "FMeshAnimSeq", [("Name", "FName"), ("Group", "FName"), ("StartFrame", "i32"), ("NumFrames", "i32"), ("Rate", "f32"), ("Notifys", "TArray")]),
    Mirror("FMeshFace", "FMeshFace", [("iWedge", "u16[3]"), ("MaterialIndex", "u16")]),
    Mirror("UAnimation", "UAnimation", [("RefBones", "TArray"), ("Moves", "TArray"), ("AnimSeqs", "TArray"), ("RawNumFrames", "i32"), ("CompFactor", "f32")]),
    Mirror(
        "USkeletalMesh",
        "USkeletalMesh",
        [
            ("FrameVerts", "i32"),
            ("Scale", "FVector"),
            ("Origin", "FVector"),
            ("RotOrigin", "FRotator"),
            ("BoundingBoxes", "TArray"),
            ("BoundingBox", "FBox"),
            ("BoundingSphere", "FPlane"),
            ("Faces", "TArray"),
            ("MeshScaleMax", "f32"),
            ("RefSkeleton", "TArray"),
            ("BoneWeightIdx", "TArray"),
            ("BoneWeights", "TArray"),
            ("LocalPoints", "TArray"),
            ("BoundingBoxSum", "FBox"),
            ("WeaponBoneIndex", "i32"),
            ("WeaponAdjust", "FCoords"),
            ("SkeletalDepth", "i32"),
            ("DefaultAnimation", "ptr"),
            ("DebugPivots", "TArray"),
            ("DebugParents", "TArray"),
            ("DebugVerts", "TArray"),
            ("DisplayBones", "i32"),
            ("DisplayInfluences", "i32"),
            ("ClassicWeaponCoords", "FCoords"),
        ],
    ),
    Mirror(
        "CFSkelHeader",
        "USkeletalMesh::CFSkelHeader",
        [
            ("CachedMesh", "ptr"),
            ("CachedAnim", "ptr"),
            ("CachedFrame", "f32"),
            ("CachedAlpha", "f32"),
            ("CachedSeq", "FName"),
            ("CacheLoaded", "i32"),
            ("CachedLinks", "ptr"),
            ("CachedPlaces", "ptr"),
            ("MeshCoords", "FCoords"),
            ("PrevBound", "FBox"),
            ("PrevRoot", "FVector"),
            ("PrevRootFrame", "f32"),
            ("Movement", "FVector"),
            ("MovementSurplus", "FVector"),
            ("bGotRoot", "u8"),
        ],
    ),
    Mirror("UGesture", "UGesture", [("Points", "TArray")], packed=True),
    Mirror(
        "UConsole",
        "UConsole",
        [
            ("Viewport", "ptr"),
            ("TopLine", "i32"),
            ("BorderSize", "i32"),
            ("ConsoleLines", "i32"),
            ("BorderLines", "i32"),
            ("BorderPixels", "i32"),
            ("ConsolePos", "f32"),
            ("ConsoleDest", "f32"),
            ("FrameX", "f32"),
            ("FrameY", "f32"),
        ],
        bits=["bNoDrawWorld", "bDrewWorld"],
        packed=True,
        private=("Viewport", "TopLine", "BorderSize", "ConsoleLines", "BorderLines", "BorderPixels", "ConsolePos", "ConsoleDest", "FrameX", "FrameY", "bNoDrawWorld", "bDrewWorld"),
    ),
    Mirror(
        "UInput",
        "UInput",
        [
            ("Viewport", "ptr"),
            ("InputKeys", "ptr"),
            ("Action", "i32"),
            ("Delta", "f32"),
            ("KeyDownTable", "u8[255]"),
        ],
        private=("InputKeys", "Action", "Delta", "KeyDownTable"),
    ),
    Mirror("FSceneNode", "FSceneNode", [("X", "i32"), ("Y", "i32"), ("XB", "i32"), ("YB", "i32")]),
    Mirror(
        "FSoundData",
        "FSoundData",
        [("Owner", "ptr")],
    ),
    Mirror(
        "USound",
        "USound",
        [
            ("Data", "FSoundData"),
            ("LipSyncData", "FLipSyncData"),
            ("OriginalSize", "i32"),
            ("Duration", "f32"),
            ("raw_NumSamples", "i32"),
            ("raw_BitsPerSample", "i32"),
            ("raw_NumChannels", "i32"),
            ("raw_SampleRate", "i32"),
            ("OpenStreams", "TArray"),
            ("Handle", "i32"),
            ("RefCount", "i32"),
            ("FileType", "FName"),
            ("CoreFlags", "i32"),
            ("WorkingFlags", "i32"),
        ],
    ),
    Mirror(
        "FLipSyncData",
        "FLipSyncData",
        [("Owner", "ptr"), ("m_iMaxAmplitude", "i32")],
        private=("m_iMaxAmplitude",),
    ),
    Mirror(
        "FWaveModInfo",
        "FWaveModInfo",
        [
            ("pSamplesPerSec", "ptr"),
            ("pAvgBytesPerSec", "ptr"),
            ("pBlockAlign", "ptr"),
            ("pBitsPerSample", "ptr"),
            ("pChannels", "ptr"),
            ("OldBitsPerSample", "u32"),
            ("pWaveDataSize", "ptr"),
            ("pMasterSize", "ptr"),
            ("SampleDataStart", "ptr"),
            ("SampleDataEnd", "ptr"),
            ("SampleDataSize", "u32"),
            ("WaveDataEnd", "ptr"),
            ("SampleLoopsNum", "i32"),
            ("pSampleLoop", "ptr"),
            ("NewDataSize", "u32"),
            ("NoiseGate", "i32"),
        ],
    ),
    Mirror("FRiffWaveHeader", "FRiffWaveHeader", [("rID", "u32"), ("ChunkLen", "u32"), ("wID", "u32")]),
    Mirror("FRiffChunkOld", "FRiffChunkOld", [("ChunkID", "u32"), ("ChunkLen", "u32")]),
    Mirror(
        "FFormatChunk",
        "FFormatChunk",
        [
            ("wFormatTag", "u16"),
            ("nChannels", "u16"),
            ("nSamplesPerSec", "u32"),
            ("nAvgBytesPerSec", "u32"),
            ("nBlockAlign", "u16"),
            ("wBitsPerSample", "u16"),
            ("cbSize", "u16"),
        ],
    ),
    Mirror(
        "FSampleChunk",
        "FSampleChunk",
        [
            ("dwManufacturer", "u32"),
            ("dwProduct", "u32"),
            ("dwSamplePeriod", "u32"),
            ("dwMIDIUnityNote", "u32"),
            ("dwMIDIPitchFraction", "u32"),
            ("dwSMPTEFormat", "u32"),
            ("dwSMPTEOffset", "u32"),
            ("cSampleLoops", "u32"),
            ("cbSamplerData", "u32"),
        ],
    ),
    Mirror(
        "FSampleLoop",
        "FSampleLoop",
        [
            ("dwIdentifier", "u32"),
            ("dwType", "u32"),
            ("dwStart", "u32"),
            ("dwEnd", "u32"),
            ("dwFraction", "u32"),
            ("dwPlayCount", "u32"),
        ],
    ),
]


def snake(name):
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", s)
    return s.lower()


def compiler_invocation():
    entries = json.loads(COMPILE_COMMANDS.read_text())
    entry = next(e for e in entries if e["file"].endswith(PROBE_FLAG_SOURCE))
    args = shlex.split(entry["command"])
    flags = []
    skip = 0
    for a in args[1:]:
        if skip:
            skip -= 1
            continue
        if a in ("-o", "-MF", "-MT", "-MQ"):
            skip = 1
            continue
        if a in ("-c", "-MD", "-MMD") or a.endswith(PROBE_FLAG_SOURCE):
            continue
        flags.append(a)
    return args[0], flags, entry["directory"]


def probe_source(mirrors):
    out = [
        '#include "EnginePrivate.h"',
        '#include "UnParticle.h"',
        '#include "UnSkeletalMesh.h"',
        '#include "UnRender.h"',
        "#include <cstddef>",
        "#include <cstdio>",
        "#include <cstring>",
        "",
        "int main() {",
    ]
    for m in mirrors:
        out.append(f'  std::printf("SIZE {m.cpp} %zu\\n", sizeof({m.cpp}));')
        if m.base:
            out += [
                "  {",
                f"    alignas(16) unsigned char buf[sizeof({m.cpp})];",
                f"    std::printf(\"BASE {m.cpp} {m.base} %zu\\n\", (size_t)((unsigned char*)static_cast<{m.base}*>(reinterpret_cast<{m.cpp}*>(buf)) - buf));",
                "  }",
            ]
        for name, _ in m.fields:
            out.append(
                f'  std::printf("OFF {m.cpp} {name} %zu\\n", (size_t)__builtin_offsetof({m.cpp}, {name}));'
            )
            out.append(
                f'  std::printf("FSIZE {m.cpp} {name} %zu\\n", sizeof((({m.cpp}*)0)->{name}));'
            )
        for name in m.bits:
            out += [
                "  {",
                f"    alignas(16) unsigned char buf[sizeof({m.cpp})];",
                "    std::memset(buf, 0, sizeof buf);",
                f"    reinterpret_cast<{m.cpp}*>(buf)->{name} = 1;",
                "    int first = -1, count = 0;",
                "    for (size_t i = 0; i < sizeof buf; i++)",
                "      for (int b = 0; b < 8; b++)",
                "        if (buf[i] & (1 << b)) { if (first < 0) first = (int)(i * 8 + b); count++; }",
                f'    std::printf("BIT {m.cpp} {name} %d %d\\n", first, count);',
                "  }",
            ]
    out += ["  return 0;", "}", ""]
    return "\n".join(out)


def run_probe(mirrors):
    compiler, flags, directory = compiler_invocation()
    with tempfile.TemporaryDirectory(prefix="hp2-engine-abi-") as tmp:
        src = Path(tmp) / "probe.cpp"
        exe = Path(tmp) / "probe"
        src.write_text(probe_source(mirrors))
        subprocess.run(
            [
                compiler,
                *flags,
                "-O0",
                "-Wno-invalid-offsetof",
                "-Wno-unused-function",
                "-fno-access-control",
                str(src),
                "-o",
                str(exe),
            ],
            cwd=directory,
            check=True,
        )
        text = subprocess.run([str(exe)], check=True, capture_output=True, text=True).stdout
    probe = {"size": {}, "base": {}, "off": {}, "fsize": {}, "bit": {}}
    for line in text.splitlines():
        p = line.split()
        if p[0] == "SIZE":
            probe["size"][p[1]] = int(p[2])
        elif p[0] == "BASE":
            probe["base"][(p[1], p[2])] = int(p[3])
        elif p[0] == "OFF":
            probe["off"][(p[1], p[2])] = int(p[3])
        elif p[0] == "FSIZE":
            probe["fsize"][(p[1], p[2])] = int(p[3])
        elif p[0] == "BIT":
            probe["bit"][(p[1], p[2])] = (int(p[3]), int(p[4]))
        else:
            raise SystemExit(f"unexpected probe line: {line}")
    return probe


def rust_type(key):
    return RUST_TYPE.get(key, key)


def render_mirror(m, probe):
    total = probe["size"][m.cpp]
    segments = []  # (offset, length, rust name, rust type, is_public)
    members = []  # (rust path, offset)
    if m.base:
        base_off = probe["base"][(m.cpp, m.base)]
        base_size = probe["size"][m.base]
        segments.append((base_off, base_size, "base", m.base, True))
        members.append(("base", base_off))
    for name, key in m.fields:
        off = probe["off"][(m.cpp, name)]
        if probe["fsize"][(m.cpp, name)] != SCALAR_SIZES[key]:
            raise SystemExit(
                f"{m.cpp}.{name}: probe size {probe['fsize'][(m.cpp, name)]} != {SCALAR_SIZES[key]}"
            )
        segments.append((off, SCALAR_SIZES[key], snake(name), rust_type(key), True))
        members.append((snake(name), off))
    bit_bytes = {}
    for name in m.bits:
        abs_bit, count = probe["bit"][(m.cpp, name)]
        if count != 1 or abs_bit < 0:
            raise SystemExit(f"{m.cpp}.{name}: bitfield probe failed ({abs_bit}, {count})")
        bit_bytes.setdefault(abs_bit // 8, []).append((abs_bit % 8, name))
    for byte in sorted(bit_bytes):
        segments.append((byte, 1, f"_bits_{byte}", "u8", False))
    segments.sort()

    body = []
    cursor = 0
    for off, length, name, rtype, public in segments:
        if off < cursor:
            raise SystemExit(f"{m.cpp}: member {name} at {off} overlaps previous member")
        if off > cursor:
            body.append(f"    _pad_{cursor}: [u8; {off - cursor}],")
        vis = "pub " if public else ""
        body.append(f"    {vis}{name}: {rtype},")
        cursor = off + length
    if total < cursor:
        raise SystemExit(f"{m.cpp}: named members extend past sizeof {total}")
    if total > cursor:
        body.append(f"    _pad_{cursor}: [u8; {total - cursor}],")

    accessors = []
    for byte in sorted(bit_bytes):
        for bit, name in bit_bytes[byte]:
            r = snake(name)
            accessors.append(
                f"""    pub fn {r}(&self) -> bool {{
        self._bits_{byte} & (1 << {bit}) != 0
    }}

    pub fn set_{r}(&mut self, value: bool) {{
        if value {{
            self._bits_{byte} |= 1 << {bit};
        }} else {{
            self._bits_{byte} &= !(1 << {bit});
        }}
    }}
"""
            )

    text = [
        f"/// Layout mirror of C++ `{m.cpp}` ({total} bytes). Only the members listed in",
        "/// the generator manifest are named; the rest is opaque padding.",
        "#[repr(C, packed(4))]" if (m.packed or m.base) else "#[repr(C)]",
        "#[derive(Clone, Copy)]",
        f"pub struct {m.rust} {{",
        *body,
        "}",
        "",
    ]
    if accessors:
        text += [f"impl {m.rust} {{", "\n\n".join(a.rstrip("\n") for a in accessors), "}", ""]
    text.append(f"const _: () = assert!(size_of::<{m.rust}>() == {total});")
    for name, off in members:
        text.append(f"const _: () = assert!(std::mem::offset_of!({m.rust}, {name}) == {off});")
    text.append("")
    return "\n".join(text)


def render_rust(probe):
    parts = [
        "// @generated by Build/gen_engine_abi.py; do not edit by hand.",
        "//",
        "// Layout mirrors of the C++ engine classes used by the Rust engine port.",
        "// Sizes, offsets, and bitfield positions come from a compiled probe of the",
        "// real Engine/Src translation unit flags; see the generator for the manifest.",
        "",
        "use std::ffi::c_void;",
        "",
        "use crate::{FCoords, FPlane, FVector};",
        "",
        "/// Layout of `TArray<T>` (`UnTemplate.h`): `T* Data; INT ArrayNum; INT ArrayMax;`.",
        "#[repr(C)]",
        "#[derive(Clone, Copy)]",
        "pub struct TArrayRaw {",
        "    pub data: *mut c_void,",
        "    pub num: i32,",
        "    pub max: i32,",
        "}",
        "",
        "/// Layout of `TRefArray<T>` (`UnTemplate.h`): `T* Data; INT ArrayNum;`, no max.",
        "#[repr(C)]",
        "#[derive(Clone, Copy)]",
        "pub struct TRefArrayRaw {",
        "    pub data: *mut c_void,",
        "    pub num: i32,",
        "    _pad_12: [u8; 4],",
        "}",
        "",
    ]
    for m in MIRRORS:
        parts.append(render_mirror(m, probe))
    parts += [
        "#[cfg(test)]",
        "mod layout_tests {",
        "    use super::*;",
        "",
        "    #[test]",
        "    fn sizes_match_cpp() {",
    ]
    for m in MIRRORS:
        parts.append(f"        assert_eq!(size_of::<{m.rust}>(), {probe['size'][m.cpp]});")
    parts += ["    }", "}", ""]
    return "\n".join(parts)


def render_cpp(probe):
    lines = [
        "// @generated by Build/gen_engine_abi.py; do not edit by hand.",
        "//",
        "// Compile-time checks that the hp2-abi engine mirrors (crates/hp2-abi/src/engine)",
        "// match these C++ engine types. Bitfield and base-subobject positions are not",
        "// expressible as static_asserts; the generator checks them with its probe.",
        "",
        '#include "EnginePrivate.h"',
        '#include "UnParticle.h"',
        '#include "UnSkeletalMesh.h"',
        '#include "UnRender.h"',
        "",
        "#if defined(__clang__)",
        '#pragma clang diagnostic ignored "-Winvalid-offsetof"',
        "#endif",
        "",
    ]
    for m in MIRRORS:
        lines.append(
            f'static_assert(sizeof({m.cpp}) == {probe["size"][m.cpp]}, "hp2-abi engine::{m.rust} must match {m.cpp} size");'
        )
        for name, _ in m.fields:
            if name in m.private:
                continue
            lines.append(
                f'static_assert(__builtin_offsetof({m.cpp}, {name}) == {probe["off"][(m.cpp, name)]}, "hp2-abi engine::{m.rust}.{snake(name)} must match {m.cpp}::{name} offset");'
            )
    lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="fail if generated files are stale")
    check = parser.parse_args().check
    probe = run_probe(MIRRORS)
    outputs = {RUST_OUT: render_rust(probe), CPP_OUT: render_cpp(probe)}
    if check:
        stale = [p for p, text in outputs.items() if not p.is_file() or p.read_text() != text]
        for p in stale:
            print(f"stale: {p.relative_to(ROOT)}")
        if stale:
            print("run: python3 Build/gen_engine_abi.py")
            sys.exit(1)
        print("generated engine ABI is up to date")
        return
    RUST_OUT.parent.mkdir(parents=True, exist_ok=True)
    for p, text in outputs.items():
        p.write_text(text)
        print(f"wrote {p.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
