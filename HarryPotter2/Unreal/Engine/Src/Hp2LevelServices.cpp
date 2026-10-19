/*=============================================================================
	Hp2LevelServices.cpp: Level services called by the Rust level port.
	Each adapter performs one operation the Rust code cannot express without
	C++ types, in the original order.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "Hp2LevelServices.h"
#include "Hp2FfiException.h"

// ---- CORE group (crates/hp2-engine/src/level.rs) ----

// HP2_ACTOR_SLOT_DUMP hooks. Weak on GCC/Clang: a null hook means the dump
// module is not linked, and the Rust port skips the capture.
#if defined(__GNUC__)
extern void HP2ActorSlotDumpBeginRawLevelActorTable( ULevelBase* Level ) __attribute__((weak));
extern void HP2ActorSlotDumpRawLevelActor( ULevelBase* Level, INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported ) __attribute__((weak));
extern void HP2ActorSlotDumpEndRawLevelActorTable( ULevelBase* Level ) __attribute__((weak));
#else
extern void HP2ActorSlotDumpBeginRawLevelActorTable( ULevelBase* Level );
extern void HP2ActorSlotDumpRawLevelActor( ULevelBase* Level, INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported );
extern void HP2ActorSlotDumpEndRawLevelActorTable( ULevelBase* Level );
#endif

// FString handles.
static void* CoreFStringNew( void ) { return new FString(); }
static void CoreFStringDelete( void* S ) { delete (FString*)S; }
static void CoreFStringCaps( void* Dst, void* Src ) { *(FString*)Dst = ((FString*)Src)->Caps(); }
static int32_t CoreFStringLen( void* S ) { return ((FString*)S)->Len(); }
static const uint32_t* CoreFStringChars( void* S ) { return (const uint32_t*)*( *(FString*)S ); }
static void CoreFStringAssignChars( void* Dst, const uint32_t* Src ) { *(FString*)Dst = (const TCHAR*)Src; }
static void CoreFStringMid( void* Dst, void* Src, int32_t Start, int32_t Count ) { *(FString*)Dst = ((FString*)Src)->Mid( Start, Count ); }
static int32_t CoreFStringInstrChars( void* S, const uint32_t* Sub ) { return ((FString*)S)->InStr( (const TCHAR*)Sub ); }
static int32_t CoreFStringInstrFString( void* S, void* Sub ) { return ((FString*)S)->InStr( *(FString*)Sub ); }
static int32_t CoreFStringEqChars( void* S, const uint32_t* Lit ) { return *(FString*)S == (const TCHAR*)Lit ? 1 : 0; }
static int32_t CoreFStringEqFString( void* A, void* B ) { return *(FString*)A == *(FString*)B ? 1 : 0; }

// Diagnostics and globals.
static void CoreDebugf( int32_t Kind, const uint32_t* Msg )
{
	debugf( Kind == 0 ? NAME_Log : NAME_Error, TEXT("%s"), (const TCHAR*)Msg );
}
static int32_t CoreIsEditor( void ) { return GIsEditor; }
static int32_t CoreCheckFailed( const char* Expr, int32_t Line )
{
	return Hp2FfiCall( [&]{ appFailAssert( (const ANSICHAR*)Expr, "HarryPotter2/Unreal/Engine/Src/UnLevel.cpp", Line ); } );
}
static void CoreObjectGetName( void* Dst, void* Obj ) { *(FString*)Dst = ((UObject*)Obj)->GetName(); }
static int32_t CoreActorTagStricmpViewportName( void* Actor, void* Viewport )
{
	return appStricmp( *((AActor*)Actor)->Tag, ((UObject*)Viewport)->GetName() );
}
static void CoreAppSystemTimeFromLevelInfo( void* Info )
{
	ALevelInfo* LevelInfo = (ALevelInfo*)Info;
	appSystemTime( LevelInfo->Year, LevelInfo->Month, LevelInfo->DayOfWeek, LevelInfo->Day, LevelInfo->Hour, LevelInfo->Minute, LevelInfo->Second, LevelInfo->Millisecond );
}
// vals: Script, Actor, Path, See, Spawn, Audio, Un, Move, Net (each already scaled by GSecondsPerCycle*1000).
static void CoreAppSprintfStats( uint32_t* Result, const float* Vals, int32_t NumMoves )
{
	appSprintf( (TCHAR*)Result, TEXT("Script=%05.1f Actor=%04.1f Path=%04.1f See=%04.1f Spawn=%04.1f Audio=%04.1f Un=%04.1f Move=%04.1f (%i) Net=%04.1f"),
		Vals[0], Vals[1], Vals[2], Vals[3], Vals[4], Vals[5], Vals[6], Vals[7], NumMoves, Vals[8] );
}
static float CoreGSecondsPerCycle( void ) { return GSecondsPerCycle; }
static float CoreScriptCyclesF32( void ) { return (FLOAT)GScriptCycles; }
static void CoreScriptCyclesClear( void ) { GScriptCycles = 0; }
static void CoreScriptEntryTagClear( void ) { GScriptEntryTag = 0; }
static void* CoreAppRandTracePhaseScopeEnter( const char* Name ) { return new FAppRandTracePhaseScope( (const ANSICHAR*)Name ); }
static void CoreAppRandTracePhaseScopeLeave( void* Scope ) { delete (FAppRandTracePhaseScope*)Scope; }

// Level actor list and level info.
static int32_t CoreActorsNum( void* Obj ) { return ((ULevel*)Obj)->Actors.Num(); }
static void* CoreActorsGet( void* Obj, int32_t I ) { return ((ULevel*)Obj)->Actors( I ); }
static void CoreActorsEmpty( void* Obj, int32_t N ) { ((ULevel*)Obj)->Actors.Empty( N ); }
static void CoreActorsAdd( void* Obj, int32_t N ) { ((ULevel*)Obj)->Actors.Add( N ); }
static void CoreActorsCountBytes( void* Obj, void* Ar ) { ((ULevel*)Obj)->Actors.CountBytes( *(FArchive*)Ar ); }
static void* CoreALevelInfoStaticClass( void ) { return ALevelInfo::StaticClass(); }
static void CoreLevelSpawnActor( void* Obj, void* Cls ) { ((ULevel*)Obj)->SpawnActor( (UClass*)Cls ); }
static void* CoreLevelSpawnBrush( void* Obj ) { return ((ULevel*)Obj)->SpawnBrush(); }
static void* CoreLevelGetLevelInfo( void* Obj ) { return ((ULevel*)Obj)->GetLevelInfo(); }
static void CoreLevelInfoSetHighDetail( void* Obj, int32_t V ) { ((ULevel*)Obj)->GetLevelInfo()->bHighDetailMode = V; }
static int32_t CoreLevelInfoHasGame( void* Obj ) { return ((ULevel*)Obj)->GetLevelInfo()->Game != NULL ? 1 : 0; }
static void CoreLevelInfoGameEventDetailChange( void* Obj ) { ((ULevel*)Obj)->GetLevelInfo()->Game->eventDetailChange(); }
static void CoreLevelDestroyActor( void* Obj, void* Actor ) { ((ULevel*)Obj)->DestroyActor( (AActor*)Actor ); }
static void CoreLevelSpawnViewActor( void* Obj, void* Viewport ) { ((ULevel*)Obj)->SpawnViewActor( (UViewport*)Viewport ); }
static int32_t CoreLevelSingleLineCheck( void* Obj, void* Hit, void* Source, const void* End, const void* Start, uint32_t Flags, const void* Extent, uint8_t NodeFlags )
{
	return ((ULevel*)Obj)->SingleLineCheck( *(FCheckResult*)Hit, (AActor*)Source, *(const FVector*)End, *(const FVector*)Start, Flags, *(const FVector*)Extent, NodeFlags );
}
static void CoreLevelReachSpecsShrink( void* Obj ) { ((ULevel*)Obj)->ReachSpecs.Shrink(); }
static int32_t CoreLevelHasClient( void* Obj ) { return ((ULevel*)Obj)->Engine->Client != NULL ? 1 : 0; }
static int32_t CoreLevelClientViewportsNum( void* Obj ) { return ((ULevel*)Obj)->Engine->Client->Viewports.Num(); }
static void* CoreLevelClientViewport( void* Obj, int32_t I ) { return ((ULevel*)Obj)->Engine->Client->Viewports( I ); }
// The ENGINE_VERSION>230 body of ULevel::PostLoad.
static void CoreLevelRetargetActorXLevels( void* Obj )
{
	ULevel* Level = (ULevel*)Obj;
	for( TObjectIterator<AActor> It; It; ++It )
		if( It->GetOuter() == Level->GetOuter() )
			It->XLevel = Level;
}
static float CoreLevelTimeSecondsGetFloat( void* Obj ) { return ((ULevel*)Obj)->TimeSeconds.GetFloat(); }

// Collision hash and brush tracker.
static void* CoreCollisionHashNew( void ) { return GNewCollisionHash(); }
static void CoreCollisionHashAddActor( void* Hash, void* Actor ) { ((FCollisionHashBase*)Hash)->AddActor( (AActor*)Actor ); }
static void CoreCollisionHashRemoveActor( void* Hash, void* Actor ) { ((FCollisionHashBase*)Hash)->RemoveActor( (AActor*)Actor, true ); }
static void CoreCollisionHashDelete( void* Hash ) { delete (FCollisionHashBase*)Hash; }
static void CoreBrushTrackerCountBytes( void* Tracker, void* Ar ) { ((FMovingBrushTrackerBase*)Tracker)->CountBytes( *(FArchive*)Ar ); }
static void CoreBrushTrackerDelete( void* Tracker ) { delete (FMovingBrushTrackerBase*)Tracker; }
static void CoreNetDriverDelete( void* Driver ) { delete (UNetDriver*)Driver; }

// AActor and derived-class access.
static int32_t CoreActorCollideActors( void* Actor ) { return ((AActor*)Actor)->bCollideActors; }
static int32_t CoreActorIsPlayer( void* Actor ) { return ((AActor*)Actor)->IsPlayer(); }
static const uint32_t* CoreActorGroupChars( void* Actor ) { return (const uint32_t*)*((AActor*)Actor)->Group; }
static void* CoreActorExcludeGameStates( void* Actor ) { return &((AActor*)Actor)->ExcludeGameStates; }
static void CoreActorSetHiddenEd( void* Actor, int32_t V ) { ((AActor*)Actor)->bHiddenEd = V; }
static void CoreActorSetInCurrentGameState( void* Actor, int32_t V ) { ((AActor*)Actor)->bInCurrentGameState = V; }
static void CoreActorEventOnResolveGameState( void* Actor ) { ((AActor*)Actor)->eventOnResolveGameState(); }
static int32_t CoreActorIsOwnedBy( void* Actor, void* Other ) { return ((AActor*)Actor)->IsOwnedBy( (AActor*)Other ); }
static int32_t CoreActorIsALevelInfo( void* Actor ) { return ((AActor*)Actor)->IsA( ALevelInfo::StaticClass() ); }
static int32_t CoreActorIsAPlayerPawn( void* Actor ) { return ((AActor*)Actor)->IsA( APlayerPawn::StaticClass() ); }
static int32_t CoreActorIsACamera( void* Actor ) { return ((AActor*)Actor)->IsA( ACamera::StaticClass() ); }
static void* CoreActorCastCamera( void* Actor ) { return Cast<ACamera>( (UObject*)Actor ); }
static int32_t CoreActorClassIsChildOf( void* Actor, void* Parent ) { return ((AActor*)Actor)->GetClass()->IsChildOf( (UClass*)Parent ); }
static uint8_t CoreModelNodeIZone1OfActor( void* Actor, int32_t Item ) { return ((AActor*)Actor)->XLevel->Model->Nodes( Item ).iZone[1]; }

// APlayerPawn access.
static void* CorePawnCurrentGameState( void* Pawn ) { return &((APlayerPawn*)Pawn)->CurrentGameState; }
static void* CorePawnGameStateMasterList( void* Pawn ) { return &((APlayerPawn*)Pawn)->GameStateMasterList; }
static int32_t CorePawnGameStateTokenLen( void* Pawn ) { return ((APlayerPawn*)Pawn)->GameStateTokenLen; }
static int32_t CorePawnIsZoneVisible( void* Pawn, uint8_t Zone ) { return ((APlayerPawn*)Pawn)->IsZoneVisible( Zone ); }
static void* CorePawnGetPlayer( void* Pawn ) { return ((APlayerPawn*)Pawn)->Player; }
static void CorePawnSetPlayer( void* Pawn, void* Player ) { ((APlayerPawn*)Pawn)->Player = (UPlayer*)Player; }

// ACamera and UViewport access.
static void* CoreCameraPlayerViewport( void* View ) { return Cast<UViewport>( ((ACamera*)View)->Player ); }
static void CoreCameraRestoreView( void* View, void* Viewport )
{
	ACamera* Cam = (ACamera*)View;
	UViewport* Vp = (UViewport*)Viewport;
	Cam->OrthoZoom = Vp->SavedOrthoZoom;
	Cam->FovAngle = Vp->SavedFovAngle;
	Cam->ShowFlags = Vp->SavedShowFlags;
	Cam->RendMap = Vp->SavedRendMap;
	Cam->Misc1 = Vp->SavedMisc1;
	Cam->Misc2 = Vp->SavedMisc2;
}
static void* CoreViewportGetActor( void* Viewport ) { return ((UViewport*)Viewport)->Actor; }
static void CoreViewportMatchActor( void* Viewport, void* Actor )
{
	UViewport* Vp = (UViewport*)Viewport;
	Vp->Actor = (APlayerPawn*)Actor;
	Vp->Actor->Player = Vp;
}
static void CoreViewportRememberActor( void* Viewport )
{
	UViewport* Vp = (UViewport*)Viewport;
	Vp->SavedOrthoZoom = Vp->Actor->OrthoZoom;
	Vp->SavedFovAngle = Vp->Actor->FovAngle;
	Vp->SavedShowFlags = Vp->Actor->ShowFlags;
	Vp->SavedRendMap = Vp->Actor->RendMap;
	Vp->SavedMisc1 = Vp->Actor->Misc1;
	Vp->SavedMisc2 = Vp->Actor->Misc2;
	Vp->Actor = NULL;
}

// FCheckResult and Model.
static void CoreCheckResultAssign( void* Dst, const void* Src ) { *(FCheckResult*)Dst = *(const FCheckResult*)Src; }
static void* CoreModelNewRoot( void* Outer, int32_t RootOutside ) { return new( (UObject*)Outer )UModel( NULL, RootOutside ); }
static void* CoreModelNewBrush( void* Outer, void* Owner ) { return new( (UObject*)Outer, TEXT("Brush") )UModel( (ABrush*)Owner, 1 ); }
static void CoreModelModify( void* Model ) { ((UModel*)Model)->Modify(); }
static void CoreModelShrinkModel( void* Model ) { ((UModel*)Model)->ShrinkModel(); }
static int32_t CoreModelFastLineCheck( void* Model, const void* End, const void* Start ) { return ((UModel*)Model)->FastLineCheck( *(const FVector*)End, *(const FVector*)Start ); }
static void CoreBrushActorSetBrush( void* Actor, void* Brush ) { ((ABrush*)Actor)->Brush = (UModel*)Brush; }

// UObject bases and flags. Super calls are qualified so they stay non-virtual.
static void* CoreUObjectGetOuter( void* Obj ) { return ((UObject*)Obj)->GetOuter(); }
static void CoreUObjectSetFlags( void* Obj, uint32_t Flags ) { ((UObject*)Obj)->SetFlags( Flags ); }
static void CoreUObjectClearFlags( void* Obj, uint32_t Flags ) { ((UObject*)Obj)->ClearFlags( Flags ); }
static void CoreUObjectSerialize( void* Obj, void* Ar ) { ((UObject*)Obj)->UObject::Serialize( *(FArchive*)Ar ); }
static void CoreUObjectDestroy( void* Obj ) { ((UObject*)Obj)->UObject::Destroy(); }
static void CoreUObjectModify( void* Obj ) { ((UObject*)Obj)->UObject::Modify(); }
static void CoreUObjectPostLoad( void* Obj ) { ((UObject*)Obj)->UObject::PostLoad(); }

// Archive.
static int32_t CoreArchiveIsLoading( void* Ar ) { return ((FArchive*)Ar)->IsLoading(); }
static int32_t CoreArchiveIsSaving( void* Ar ) { return ((FArchive*)Ar)->IsSaving(); }
static int32_t CoreArchiveIsTrans( void* Ar ) { return ((FArchive*)Ar)->IsTrans(); }
static int32_t CoreArchiveVer( void* Ar ) { return ((FArchive*)Ar)->Ver(); }
static int32_t CoreArchiveTell( void* Ar ) { return ((FArchive*)Ar)->Tell(); }
static int32_t CoreArchiveSupportsCompactIndexTrace( void* Ar ) { return ((FArchive*)Ar)->SupportsActorSlotCompactIndexTrace(); }
static void* CoreArchiveSetCompactIndexTrace( void* Ar, void* Trace )
{
	return ((FArchive*)Ar)->SetActorSlotCompactIndexTrace( (FActorSlotCompactIndexTrace*)Trace );
}
static void CoreArchiveSerializeI32( void* Ar, int32_t* Value ) { *(FArchive*)Ar << *(INT*)Value; }
static void CoreArchiveSerializeF32( void* Ar, float* Value ) { *(FArchive*)Ar << *(FLOAT*)Value; }
static void CoreArchiveSerializeActors( void* Ar, void* Obj ) { *(FArchive*)Ar << ((ULevel*)Obj)->Actors; }
static void CoreArchiveSerializeActorSlot( void* Ar, void* Obj, int32_t I ) { *(FArchive*)Ar << ((ULevel*)Obj)->Actors( I ); }
static void CoreArchiveSerializeUrl( void* Ar, void* Obj ) { *(FArchive*)Ar << ((ULevel*)Obj)->URL; }
static void CoreArchiveSerializeNetDriver( void* Ar, void* Slot ) { *(FArchive*)Ar << *(UNetDriver**)Slot; }
static void CoreArchiveSerializeModel( void* Ar, void* Slot ) { *(FArchive*)Ar << *(UModel**)Slot; }
static void CoreArchiveSerializeReachSpecs( void* Ar, void* Obj ) { *(FArchive*)Ar << ((ULevel*)Obj)->ReachSpecs; }
static void CoreArchiveSerializeActorPtr( void* Ar, void* Slot ) { *(FArchive*)Ar << *(AActor**)Slot; }
static void CoreArchiveSerializeTextBuffer( void* Ar, void* Slot ) { *(FArchive*)Ar << *(UTextBuffer**)Slot; }
static void CoreArchiveSerializeTravelInfo( void* Ar, void* Obj ) { *(FArchive*)Ar << ((ULevel*)Obj)->TravelInfo; }
// Version 61-62 layout: name/item string arrays rebuilt into the TravelInfo map.
static void CoreArchiveSerializeTravelInfoLegacy( void* Ar, void* Obj )
{
	ULevel* Level = (ULevel*)Obj;
	TArray<FString> Names, Items;
	*(FArchive*)Ar << Names << Items;
	Level->TravelInfo = TMap<FString,FString>();
	for( INT i=0; i<Names.Num(); i++ )
		Level->TravelInfo.Set( *Names(i), *Items(i) );
}
static void CoreArchivePreload( void* Ar, void* Model ) { ((FArchive*)Ar)->Preload( (UModel*)Model ); }

// HP2_ACTOR_SLOT_DUMP. getenv is called by the Rust port on every Serialize.
static const char* CoreActorSlotDumpEnvPath( void ) { return (const char*)getenv( "HP2_ACTOR_SLOT_DUMP" ); }
static int32_t CoreActorSlotDumpHooksLinked( void )
{
#if defined(__GNUC__)
	return HP2ActorSlotDumpBeginRawLevelActorTable != NULL && HP2ActorSlotDumpRawLevelActor != NULL && HP2ActorSlotDumpEndRawLevelActorTable != NULL ? 1 : 0;
#else
	return 1;
#endif
}
static void CoreActorSlotDumpBegin( void* Obj ) { HP2ActorSlotDumpBeginRawLevelActorTable( (ULevelBase*)Obj ); }
static void CoreActorSlotDumpRawActor( void* Obj, int32_t Slot, void* Actor, int32_t RefBefore, int32_t RefAfter, int32_t Index, int32_t Captured, int32_t Supported )
{
	HP2ActorSlotDumpRawLevelActor( (ULevelBase*)Obj, Slot, (AActor*)Actor, RefBefore, RefAfter, Index, Captured, Supported );
}
static void CoreActorSlotDumpEnd( void* Obj ) { HP2ActorSlotDumpEndRawLevelActorTable( (ULevelBase*)Obj ); }

static constexpr Hp2LevelCoreServices MakeCoreServices(void)
{
	Hp2LevelCoreServices s = {};
	s.fstring_new = CoreFStringNew;
	s.fstring_delete = CoreFStringDelete;
	s.fstring_caps = CoreFStringCaps;
	s.fstring_len = CoreFStringLen;
	s.fstring_chars = CoreFStringChars;
	s.fstring_assign_chars = CoreFStringAssignChars;
	s.fstring_mid = CoreFStringMid;
	s.fstring_instr_chars = CoreFStringInstrChars;
	s.fstring_instr_fstring = CoreFStringInstrFString;
	s.fstring_eq_chars = CoreFStringEqChars;
	s.fstring_eq_fstring = CoreFStringEqFString;
	s.debug_log = CoreDebugf;
	s.is_editor = CoreIsEditor;
	s.check_failed = CoreCheckFailed;
	s.object_get_name = CoreObjectGetName;
	s.actor_tag_stricmp_viewport_name = CoreActorTagStricmpViewportName;
	s.app_system_time_from_level_info = CoreAppSystemTimeFromLevelInfo;
	s.app_sprintf_stats = CoreAppSprintfStats;
	s.gseconds_per_cycle = CoreGSecondsPerCycle;
	s.script_cycles_f32 = CoreScriptCyclesF32;
	s.script_cycles_clear = CoreScriptCyclesClear;
	s.script_entry_tag_clear = CoreScriptEntryTagClear;
	s.app_rand_trace_phase_scope_enter = CoreAppRandTracePhaseScopeEnter;
	s.app_rand_trace_phase_scope_leave = CoreAppRandTracePhaseScopeLeave;
	s.actors_num = CoreActorsNum;
	s.actors_get = CoreActorsGet;
	s.actors_empty = CoreActorsEmpty;
	s.actors_add = CoreActorsAdd;
	s.actors_count_bytes = CoreActorsCountBytes;
	s.alevel_info_static_class = CoreALevelInfoStaticClass;
	s.level_spawn_actor = CoreLevelSpawnActor;
	s.level_spawn_brush = CoreLevelSpawnBrush;
	s.level_get_level_info = CoreLevelGetLevelInfo;
	s.level_info_set_high_detail = CoreLevelInfoSetHighDetail;
	s.level_info_has_game = CoreLevelInfoHasGame;
	s.level_info_game_event_detail_change = CoreLevelInfoGameEventDetailChange;
	s.level_destroy_actor = CoreLevelDestroyActor;
	s.level_spawn_view_actor = CoreLevelSpawnViewActor;
	s.level_single_line_check = CoreLevelSingleLineCheck;
	s.level_reach_specs_shrink = CoreLevelReachSpecsShrink;
	s.level_has_client = CoreLevelHasClient;
	s.level_client_viewports_num = CoreLevelClientViewportsNum;
	s.level_client_viewport = CoreLevelClientViewport;
	s.level_retarget_actor_xlevels = CoreLevelRetargetActorXLevels;
	s.level_time_seconds_get_float = CoreLevelTimeSecondsGetFloat;
	s.collision_hash_new = CoreCollisionHashNew;
	s.collision_hash_add_actor = CoreCollisionHashAddActor;
	s.collision_hash_remove_actor = CoreCollisionHashRemoveActor;
	s.collision_hash_delete = CoreCollisionHashDelete;
	s.brush_tracker_count_bytes = CoreBrushTrackerCountBytes;
	s.brush_tracker_delete = CoreBrushTrackerDelete;
	s.net_driver_delete = CoreNetDriverDelete;
	s.actor_collide_actors = CoreActorCollideActors;
	s.actor_is_player = CoreActorIsPlayer;
	s.actor_group_chars = CoreActorGroupChars;
	s.actor_exclude_game_states = CoreActorExcludeGameStates;
	s.actor_set_hidden_ed = CoreActorSetHiddenEd;
	s.actor_set_in_current_game_state = CoreActorSetInCurrentGameState;
	s.actor_event_on_resolve_game_state = CoreActorEventOnResolveGameState;
	s.actor_is_owned_by = CoreActorIsOwnedBy;
	s.actor_is_a_alevel_info = CoreActorIsALevelInfo;
	s.actor_is_a_aplayer_pawn = CoreActorIsAPlayerPawn;
	s.actor_is_a_camera = CoreActorIsACamera;
	s.actor_cast_camera = CoreActorCastCamera;
	s.actor_class_is_child_of = CoreActorClassIsChildOf;
	s.model_node_izone1_of_actor = CoreModelNodeIZone1OfActor;
	s.pawn_current_game_state = CorePawnCurrentGameState;
	s.pawn_game_state_master_list = CorePawnGameStateMasterList;
	s.pawn_game_state_token_len = CorePawnGameStateTokenLen;
	s.pawn_is_zone_visible = CorePawnIsZoneVisible;
	s.pawn_get_player = CorePawnGetPlayer;
	s.pawn_set_player = CorePawnSetPlayer;
	s.camera_player_viewport = CoreCameraPlayerViewport;
	s.camera_restore_view = CoreCameraRestoreView;
	s.viewport_get_actor = CoreViewportGetActor;
	s.viewport_match_actor = CoreViewportMatchActor;
	s.viewport_remember_actor = CoreViewportRememberActor;
	s.check_result_assign = CoreCheckResultAssign;
	s.model_new_root = CoreModelNewRoot;
	s.model_new_brush = CoreModelNewBrush;
	s.model_modify = CoreModelModify;
	s.model_shrink_model = CoreModelShrinkModel;
	s.model_fast_line_check = CoreModelFastLineCheck;
	s.brush_actor_set_brush = CoreBrushActorSetBrush;
	s.uobject_get_outer = CoreUObjectGetOuter;
	s.uobject_set_flags = CoreUObjectSetFlags;
	s.uobject_clear_flags = CoreUObjectClearFlags;
	s.uobject_serialize = CoreUObjectSerialize;
	s.uobject_destroy = CoreUObjectDestroy;
	s.uobject_modify = CoreUObjectModify;
	s.uobject_post_load = CoreUObjectPostLoad;
	s.archive_is_loading = CoreArchiveIsLoading;
	s.archive_is_saving = CoreArchiveIsSaving;
	s.archive_is_trans = CoreArchiveIsTrans;
	s.archive_ver = CoreArchiveVer;
	s.archive_tell = CoreArchiveTell;
	s.archive_supports_compact_index_trace = CoreArchiveSupportsCompactIndexTrace;
	s.archive_set_compact_index_trace = CoreArchiveSetCompactIndexTrace;
	s.archive_serialize_i32 = CoreArchiveSerializeI32;
	s.archive_serialize_f32 = CoreArchiveSerializeF32;
	s.archive_serialize_actors = CoreArchiveSerializeActors;
	s.archive_serialize_actor_slot = CoreArchiveSerializeActorSlot;
	s.archive_serialize_url = CoreArchiveSerializeUrl;
	s.archive_serialize_net_driver = CoreArchiveSerializeNetDriver;
	s.archive_serialize_model = CoreArchiveSerializeModel;
	s.archive_serialize_reach_specs = CoreArchiveSerializeReachSpecs;
	s.archive_serialize_actor_ptr = CoreArchiveSerializeActorPtr;
	s.archive_serialize_text_buffer = CoreArchiveSerializeTextBuffer;
	s.archive_serialize_travel_info = CoreArchiveSerializeTravelInfo;
	s.archive_serialize_travel_info_legacy = CoreArchiveSerializeTravelInfoLegacy;
	s.archive_preload = CoreArchivePreload;
	s.actor_slot_dump_env_path = CoreActorSlotDumpEnvPath;
	s.actor_slot_dump_hooks_linked = CoreActorSlotDumpHooksLinked;
	s.actor_slot_dump_begin = CoreActorSlotDumpBegin;
	s.actor_slot_dump_raw_actor = CoreActorSlotDumpRawActor;
	s.actor_slot_dump_end = CoreActorSlotDumpEnd;
	return s;
}
const Hp2LevelCoreServices GHp2LevelCoreServices = MakeCoreServices();
// Compile-time proof that the table is constant-initialized; fails to compile otherwise.
[[maybe_unused]] static constexpr Hp2LevelCoreServices kCoreServicesConstantCheck = MakeCoreServices();


// ---- PERSIST group (crates/hp2-engine/src/level_persist.rs) ----

static int32_t PersistIsEditor( void )
{
	return GIsEditor;
}

static int32_t CopyOutStr( const TCHAR* Src, uint32_t* Out, int32_t Cap )
{
	int32_t Len = 0;
	while( Src[Len] )
		Len++;
	if( Len + 1 <= Cap )
		memcpy( (TCHAR*)Out, Src, (Len + 1) * sizeof(TCHAR) );
	return Len + 1;
}

static int32_t PersistGSysMapPath( uint32_t* Out, int32_t Cap )
{
	FString S = GSys->MapPath;
	return CopyOutStr( *S, Out, Cap );
}

static int32_t PersistGSysSaveSlotPath( uint32_t* Out, int32_t Cap )
{
	FString S = GSys->SaveSlotPath;
	return CopyOutStr( *S, Out, Cap );
}

static int32_t PersistUrlDefaultMapExt( uint32_t* Out, int32_t Cap )
{
	FString S = FURL::DefaultMapExt;
	return CopyOutStr( *S, Out, Cap );
}

static int32_t PersistUrlDefaultSaveExt( uint32_t* Out, int32_t Cap )
{
	FString S = FURL::DefaultSaveExt;
	return CopyOutStr( *S, Out, Cap );
}

static int32_t PersistAppPathLeaf( const uint32_t* Path, uint32_t* Out, int32_t Cap )
{
	return CopyOutStr( appPathLeaf( (const TCHAR*)Path ), Out, Cap );
}

static int32_t PersistFStringInStr( const uint32_t* S, const uint32_t* Sub )
{
	FString A = (const TCHAR*)S;
	FString B = (const TCHAR*)Sub;
	return A.InStr( B );
}

static int32_t PersistFStringLeft( const uint32_t* S, int32_t N, uint32_t* Out, int32_t Cap )
{
	FString A = (const TCHAR*)S;
	FString R = A.Left( N );
	return CopyOutStr( *R, Out, Cap );
}

static int32_t PersistFStringPathJoin( const uint32_t* A, const uint32_t* B, uint32_t* Out, int32_t Cap )
{
	FString SA = (const TCHAR*)A;
	FString SB = (const TCHAR*)B;
	FString R = SA * SB;
	return CopyOutStr( *R, Out, Cap );
}

static int32_t PersistFStringAppend( const uint32_t* A, const uint32_t* B, uint32_t* Out, int32_t Cap )
{
	FString SA = (const TCHAR*)A;
	FString SB = (const TCHAR*)B;
	FString R = SA + SB;
	return CopyOutStr( *R, Out, Cap );
}

static int32_t PersistFStringNe( const uint32_t* A, const uint32_t* B )
{
	FString SA = (const TCHAR*)A;
	FString SB = (const TCHAR*)B;
	return SA != SB ? 1 : 0;
}

static int32_t PersistAppStrcmp( const uint32_t* A, const uint32_t* B )
{
	return appStrcmp( (const TCHAR*)A, (const TCHAR*)B );
}

static int32_t PersistLevelActorsNum( void* Level )
{
	return ((ULevel*)Level)->Actors.Num();
}

static void* PersistLevelActorAt( void* Level, int32_t I )
{
	return ((ULevel*)Level)->Actors( I );
}

static int32_t PersistActorBPersistent( void* Actor )
{
	return ((AActor*)Actor)->bPersistent;
}

static int32_t PersistActorName( void* Actor, uint32_t* Out, int32_t Cap )
{
	return CopyOutStr( ((AActor*)Actor)->GetName(), Out, Cap );
}

static void PersistActorSetBScriptInitialized( void* Actor, int32_t V )
{
	((AActor*)Actor)->bScriptInitialized = V;
}

static void* PersistLoadMapNew( void )
{
	return new TMultiMap<FName,AActor*>();
}

static void PersistLoadMapDelete( void* Map )
{
	delete (TMultiMap<FName,AActor*>*)Map;
}

static void PersistLoadMapAdd( void* Map, const uint32_t* Name, void* Actor )
{
	((TMultiMap<FName,AActor*>*)Map)->Add( (const TCHAR*)Name, (AActor*)Actor );
}

static void* PersistLoadMapFind( void* Map, const uint32_t* Name )
{
	AActor** ppAct = ((TMultiMap<FName,AActor*>*)Map)->Find( (const TCHAR*)Name );
	return ppAct ? *ppAct : NULL;
}

static void PersistLoadMapRemove( void* Map, const uint32_t* Name )
{
	((TMultiMap<FName,AActor*>*)Map)->Remove( (const TCHAR*)Name );
}

static void* PersistLoadMapIterBegin( void* Map )
{
	return new TMultiMap<FName,AActor*>::TIterator( *(TMultiMap<FName,AActor*>*)Map );
}

static int32_t PersistLoadMapIterValid( void* It )
{
	return (UBOOL)*(TMultiMap<FName,AActor*>::TIterator*)It ? 1 : 0;
}

static void* PersistLoadMapIterValue( void* It )
{
	return ((TMultiMap<FName,AActor*>::TIterator*)It)->Value();
}

static void PersistLoadMapIterNext( void* It )
{
	++*(TMultiMap<FName,AActor*>::TIterator*)It;
}

static void PersistLoadMapIterEnd( void* It )
{
	delete (TMultiMap<FName,AActor*>::TIterator*)It;
}

static void* PersistArchiveWriterCreate( const uint32_t* Path )
{
	return (void*)GFileManager->CreateFileWriter( (const TCHAR*)Path );
}

static void* PersistArchiveReaderCreate( const uint32_t* Path )
{
	return (void*)GFileManager->CreateFileReader( (const TCHAR*)Path );
}

static void PersistArchiveClose( void* Ar )
{
	delete (FArchive*)Ar;
}

static int32_t PersistArchiveTell( void* Ar )
{
	return ((FArchive*)Ar)->Tell();
}

static void PersistArchiveSeek( void* Ar, int32_t Pos )
{
	((FArchive*)Ar)->Seek( Pos );
}

static int32_t PersistArchiveTotalSize( void* Ar )
{
	return ((FArchive*)Ar)->TotalSize();
}

static int32_t PersistArchiveGetError( void* Ar )
{
	return ((FArchive*)Ar)->GetError();
}

static void PersistArchiveSerializeBytes( void* Ar, void* Data, int32_t Len )
{
	((FArchive*)Ar)->ByteOrderSerialize( Data, Len );
}

static void PersistArchiveSerializeInt( void* Ar, int32_t* V )
{
	INT I = *V;
	*(FArchive*)Ar << I;
	*V = I;
}

static void PersistArchiveSerializeSqword( void* Ar, int64_t* V )
{
	SQWORD S = (SQWORD)*V;
	*(FArchive*)Ar << S;
	*V = (int64_t)S;
}

static int32_t PersistFileManagerFileSize( const uint32_t* Path )
{
	return GFileManager->FileSize( (const TCHAR*)Path );
}

static int64_t PersistFileManagerGlobalTime( const uint32_t* Path )
{
	return (int64_t)GFileManager->GetGlobalTime( (const TCHAR*)Path );
}

static void PersistActorSerializePersistence( void* Ar, void* Actor )
{
	((AActor*)Actor)->SerializePersistence( *(FArchive*)Ar );
}

static void PersistActorDestroy( void* Level, void* Actor )
{
	((ULevel*)Level)->DestroyActor( (AActor*)Actor );
}

static void PersistDebugLog( int32_t Name, const uint32_t* Msg )
{
	debugf( Name == 0 ? NAME_Log : NAME_Error, TEXT("%s"), (const TCHAR*)Msg );
}

static constexpr Hp2LevelPersistServices MakePersistServices( void )
{
	Hp2LevelPersistServices s{};
	s.is_editor = PersistIsEditor;
	s.gsys_map_path = PersistGSysMapPath;
	s.gsys_save_slot_path = PersistGSysSaveSlotPath;
	s.url_default_map_ext = PersistUrlDefaultMapExt;
	s.url_default_save_ext = PersistUrlDefaultSaveExt;
	s.app_path_leaf = PersistAppPathLeaf;
	s.fstring_in_str = PersistFStringInStr;
	s.fstring_left = PersistFStringLeft;
	s.fstring_path_join = PersistFStringPathJoin;
	s.fstring_append = PersistFStringAppend;
	s.fstring_ne = PersistFStringNe;
	s.app_strcmp = PersistAppStrcmp;
	s.level_actors_num = PersistLevelActorsNum;
	s.level_actor_at = PersistLevelActorAt;
	s.actor_b_persistent = PersistActorBPersistent;
	s.actor_name = PersistActorName;
	s.actor_set_b_script_initialized = PersistActorSetBScriptInitialized;
	s.load_map_new = PersistLoadMapNew;
	s.load_map_delete = PersistLoadMapDelete;
	s.load_map_add = PersistLoadMapAdd;
	s.load_map_find = PersistLoadMapFind;
	s.load_map_remove = PersistLoadMapRemove;
	s.load_map_iter_begin = PersistLoadMapIterBegin;
	s.load_map_iter_valid = PersistLoadMapIterValid;
	s.load_map_iter_value = PersistLoadMapIterValue;
	s.load_map_iter_next = PersistLoadMapIterNext;
	s.load_map_iter_end = PersistLoadMapIterEnd;
	s.archive_writer_create = PersistArchiveWriterCreate;
	s.archive_reader_create = PersistArchiveReaderCreate;
	s.archive_close = PersistArchiveClose;
	s.archive_tell = PersistArchiveTell;
	s.archive_seek = PersistArchiveSeek;
	s.archive_total_size = PersistArchiveTotalSize;
	s.archive_get_error = PersistArchiveGetError;
	s.archive_serialize_bytes = PersistArchiveSerializeBytes;
	s.archive_serialize_int = PersistArchiveSerializeInt;
	s.archive_serialize_sqword = PersistArchiveSerializeSqword;
	s.file_manager_file_size = PersistFileManagerFileSize;
	s.file_manager_global_time = PersistFileManagerGlobalTime;
	s.actor_serialize_persistence = PersistActorSerializePersistence;
	s.actor_destroy = PersistActorDestroy;
	s.debug_log = PersistDebugLog;
	return s;
}

const Hp2LevelPersistServices GHp2LevelPersistServices = MakePersistServices();
// Compile-time proof that the table is constant-initialized; fails to compile otherwise.
[[maybe_unused]] static constexpr Hp2LevelPersistServices kPersistServicesConstantCheck = MakePersistServices();

// ---- NET group (crates/hp2-engine/src/level_net.rs) ----

static void NetDebugfLog( const uint32_t* Text ) { debugf( TEXT("%s"), (const TCHAR*)Text ); }
static void NetDebugfDevNet( const uint32_t* Text ) { debugf( NAME_DevNet, TEXT("%s"), (const TCHAR*)Text ); }
static void NetDebugfNetComeGo( const uint32_t* Text ) { debugf( NAME_NetComeGo, TEXT("%s"), (const TCHAR*)Text ); }
static void NetOutputDeviceLog( void* Ar, const uint32_t* Text ) { ((FOutputDevice*)Ar)->Log( (const TCHAR*)Text ); }
static void NetOutputDeviceLogf( void* Ar, const uint32_t* Text ) { ((FOutputDevice*)Ar)->Logf( TEXT("%s"), (const TCHAR*)Text ); }
static void NetConnectionLog( void* Conn, const uint32_t* Text ) { ((UNetConnection*)Conn)->Logf( TEXT("%s"), (const TCHAR*)Text ); }
static int32_t NetCheckFailed( const int8_t* Expr, const int8_t* File, int32_t Line ) { return Hp2FfiCall( [&]{ appFailAssert( (const ANSICHAR*)Expr, (const ANSICHAR*)File, Line ); } ); }
static int32_t NetAppErrorf( const uint32_t* Text ) { return Hp2FfiCall( [&]{ appErrorf( TEXT("%s"), (const TCHAR*)Text ); } ); }
static void NetLocalizeErrorAssign( void* Level, void* Error, const int8_t* Key ) { (void)Level; *(FString*)Error = LocalizeError( (const ANSICHAR*)Key ); }
static const uint32_t* NetUObjectGetName( void* Obj ) { return (const uint32_t*)((UObject*)Obj)->GetName(); }
static void* NetUObjectGetFullName( void* Obj ) { return new FString( ((UObject*)Obj)->GetFullName() ); }
static void* NetUObjectGetOuter( void* Obj ) { return ((UObject*)Obj)->GetOuter(); }
static void* NetLevelGetLinker( void* Level ) { return ((ULevel*)Level)->GetLinker(); }
static const uint32_t* NetAppTimestamp( void ) { return (const uint32_t*)appTimestamp(); }
static uint32_t NetAppCycles( void ) { return (uint32_t)appCycles(); }
static int32_t NetAppAtoi( const uint32_t* Text ) { return appAtoi( (const TCHAR*)Text ); }
static const uint32_t* NetAppStrchr( const uint32_t* Text, uint32_t Ch ) { return (const uint32_t*)appStrchr( (const TCHAR*)Text, (TCHAR)Ch ); }
static void* NetFStringNew( void ) { return new FString(); }
static void NetFStringDelete( void* H ) { delete (FString*)H; }
static const uint32_t* NetFStringText( void* H ) { return (const uint32_t*)**(FString*)H; }
static void NetFStringAssign( void* Dst, void* Src ) { *(FString*)Dst = *(FString*)Src; }
static void NetFStringAppendText( void* H, const uint32_t* Text ) { *(FString*)H += (const TCHAR*)Text; }
static int32_t NetFStringNeText( void* H, const uint32_t* Text ) { return *(FString*)H != (const TCHAR*)Text; }
static void* NetFStringRight( void* H, int32_t N ) { return new FString( ((FString*)H)->Right( N ) ); }
static void* NetFStringLeft( void* H, int32_t N ) { return new FString( ((FString*)H)->Left( N ) ); }
static void* NetFStringMid( void* H, int32_t N ) { return new FString( ((FString*)H)->Mid( N ) ); }
static void* NetFStringAdd( void* A, void* B ) { return new FString( *(FString*)A + *(FString*)B ); }
static int32_t NetFStringCapsInStr( void* H, const uint32_t* Needle ) { return ((FString*)H)->Caps().InStr( (const TCHAR*)Needle ); }
static void* NetFURLNew( void ) { return new FURL(); }
static void* NetFURLNewTravel( const uint32_t* Text, int32_t Travel ) { return new FURL( NULL, (const TCHAR*)Text, (ETravelType)Travel ); }
static void NetFURLDelete( void* H ) { delete (FURL*)H; }
static void* NetFURLMap( void* H ) { return &((FURL*)H)->Map; }
static int32_t NetParseCommand( const uint32_t** Stream, const uint32_t* Keyword ) { return ParseCommand( reinterpret_cast<const TCHAR**>(Stream), (const TCHAR*)Keyword ); }
static int32_t NetParseTokenFString( const uint32_t** Stream, void* Token, int32_t Escape ) { return ParseToken( *reinterpret_cast<const TCHAR**>(Stream), *(FString*)Token, Escape ); }
static int32_t NetParseTokenBuf( const uint32_t** Stream, uint32_t* Buf, int32_t Max, int32_t Escape ) { return ParseToken( *reinterpret_cast<const TCHAR**>(Stream), reinterpret_cast<TCHAR*>(Buf), Max, Escape ); }
static int32_t NetParseInt( const uint32_t* Text, const uint32_t* Key, int32_t* Out ) { return Parse( (const TCHAR*)Text, (const TCHAR*)Key, *Out ); }
static int32_t NetParseText( const uint32_t* Text, const uint32_t* Key, uint32_t* Buf, int32_t Max ) { return Parse( (const TCHAR*)Text, (const TCHAR*)Key, reinterpret_cast<TCHAR*>(Buf), Max ); }
static int32_t NetParseGuid( const uint32_t* Text, const uint32_t* Key, uint32_t* Out ) { return Parse( (const TCHAR*)Text, (const TCHAR*)Key, *(FGuid*)Out ); }
static int32_t NetLevelExecNet( void* Drv, const uint32_t* Cmd, void* Ar ) { return ((UNetDriver*)Drv)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar ); }
static int32_t NetLevelExecDemo( void* Drv, const uint32_t* Cmd, void* Ar ) { return ((UNetDriver*)Drv)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar ); }
static void* NetStaticLoadClass( int32_t Base, const uint32_t* Name )
{
	if( Base == 0 )
		return UObject::StaticLoadClass( UNetDriver::StaticClass(), NULL, (const TCHAR*)Name, NULL, LOAD_NoFail, NULL );
	return UObject::StaticLoadClass( AActor::StaticClass(), NULL, (const TCHAR*)Name, NULL, LOAD_NoFail, NULL );
}
static void* NetConstructNetDriver( void* Cls ) { return ConstructObject<UNetDriver>( (UClass*)Cls ); }
static void* NetStaticConstructNetDriver( void* Cls ) { return (UNetDriver*)UObject::StaticConstructObject( (UClass*)Cls ); }
static int32_t NetNetDriverInitListen( void* Drv, void* Level, void* Url, void* Error ) { return ((UNetDriver*)Drv)->InitListen( (ULevel*)Level, *(FURL*)Url, *(FString*)Error ); }
static void NetNetDriverDelete( void* Drv ) { delete (UNetDriver*)Drv; }
static void* NetNetDriverServerConnection( void* Drv ) { return ((UNetDriver*)Drv)->ServerConnection; }
static float NetNetDriverServerTravelPause( void* Drv ) { return ((UNetDriver*)Drv)->ServerTravelPause; }
static int32_t NetNetDriverMaxClientRate( void* Drv ) { return ((UNetDriver*)Drv)->MaxClientRate; }
static void* NetNetDriverMasterMap( void* Drv ) { return ((UNetDriver*)Drv)->MasterMap; }
static void* NetCastGameEngine( void* Engine ) { return CastChecked<UGameEngine>( (UEngine*)Engine ); }
static void* NetGameEngineGPendingLevel( void* Ge ) { return ((UGameEngine*)Ge)->GPendingLevel; }
static void NetGameEngineSetGPendingLevel( void* Ge, void* V ) { ((UGameEngine*)Ge)->GPendingLevel = (decltype(((UGameEngine*)Ge)->GPendingLevel))V; }
static void NetGameEngineCancelPending( void* Ge ) { ((UGameEngine*)Ge)->CancelPending(); }
static void NetGameBuildServerMasterMap( void* Ge, void* Drv, void* Level ) { ((UGameEngine*)Ge)->BuildServerMasterMap( (UNetDriver*)Drv, (ULevel*)Level ); }
static int32_t NetGameEngineServerActorsNum( void* Ge ) { return ((UGameEngine*)Ge)->ServerActors.Num(); }
static const uint32_t* NetGameEngineServerActorText( void* Ge, int32_t I ) { return (const uint32_t*)*((UGameEngine*)Ge)->ServerActors( I ); }
static void* NetDemoPlayPendingLevelNew( void* Ge, void* Url ) { return new UDemoPlayPendingLevel( (UGameEngine*)Ge, *(FURL*)Url ); }
static void* NetPendingLevelDemoRecDriver( void* P ) { return ((UDemoPlayPendingLevel*)P)->DemoRecDriver; }
static const uint32_t* NetPendingLevelErrorText( void* P ) { return (const uint32_t*)*((UDemoPlayPendingLevel*)P)->Error; }
static void NetPendingLevelDelete( void* Ge ) { delete ((UGameEngine*)Ge)->GPendingLevel; }
static void* NetEngineClient( void* Engine ) { return ((UEngine*)Engine)->Client; }
static int32_t NetEngineClientViewportsNum( void* Client ) { return ((UClient*)Client)->Viewports.Num(); }
static void* NetEngineClientViewport( void* Client, int32_t I ) { return ((UClient*)Client)->Viewports( I ); }
static void NetEngineSetClientTravel( void* Engine, void* Player, const uint32_t* Url, int32_t Items, int32_t Travel ) { ((UEngine*)Engine)->SetClientTravel( (UPlayer*)Player, (const TCHAR*)Url, Items, (ETravelType)Travel ); }
static int32_t NetEngineChallengeResponse( void* Engine, uint32_t Challenge ) { return ((UEngine*)Engine)->ChallengeResponse( (INT)Challenge ); }
static void* NetLevelSpawnActor( void* Level, void* Cls ) { return ((ULevel*)Level)->SpawnActor( (UClass*)Cls ); }
static void* NetLevelUrl( void* Level ) { return &((ULevel*)Level)->URL; }
static void* NetLevelGetLevelInfo( void* Level ) { return ((ULevel*)Level)->GetLevelInfo(); }
static void* NetSpawnPlayActor( void* Level, void* Conn, int32_t Role, void* Url, void* Error ) { return ((ULevel*)Level)->SpawnPlayActor( (UPlayer*)Conn, (ENetRole)Role, *(FURL*)Url, *(FString*)Error ); }
static void NetLevelWelcomePlayer( void* Level, void* Conn ) { ((ULevel*)Level)->WelcomePlayer( (UNetConnection*)Conn ); }
static void* NetLevelInfoGame( void* Li ) { return ((ALevelInfo*)Li)->Game; }
static void* NetLevelInfoNextUrl( void* Li ) { return &((ALevelInfo*)Li)->NextURL; }
static int32_t NetLevelInfoBLonePlayer( void* Li ) { return ((ALevelInfo*)Li)->bLonePlayer; }
static void NetLevelInfoSetNextSwitchCountdown( void* Li, float V ) { ((ALevelInfo*)Li)->NextSwitchCountdown = V; }
static void NetLevelInfoSetNetMode( void* Li, uint8_t V ) { ((ALevelInfo*)Li)->NetMode = V; }
static int32_t NetGameInfoBWorldLog( void* Game ) { return ((AGameInfo*)Game)->bWorldLog; }
static void NetGamePreLogin( void* Game, const uint32_t* Options, const uint32_t* Address, void* Error, void* FailCode )
{
	((AGameInfo*)Game)->eventPreLogin( FString( (const TCHAR*)Options ), FString( (const TCHAR*)Address ), *(FString*)Error, *(FString*)FailCode );
}
static void* NetActorGetClass( void* Actor ) { return ((AActor*)Actor)->GetClass(); }
static void* NetTFieldIterNew( void* Cls ) { return new TFieldIterator<UProperty>( (UClass*)Cls ); }
static int32_t NetTFieldIterValid( void* It ) { return (int32_t)(UBOOL)*(TFieldIterator<UProperty>*)It; }
static void NetTFieldIterNext( void* It ) { ++*(TFieldIterator<UProperty>*)It; }
static void NetTFieldIterDelete( void* It ) { delete (TFieldIterator<UProperty>*)It; }
static int32_t NetUPropertyNameStricmp( void* It, const uint32_t* Name ) { return appStricmp( (*(TFieldIterator<UProperty>*)It)->GetName(), (const TCHAR*)Name ); }
static int32_t NetUPropertyIsConfig( void* It ) { return ((*(TFieldIterator<UProperty>*)It)->PropertyFlags & CPF_Config) != 0; }
static void NetUPropertyImportText( void* It, const uint32_t* Text, void* Obj )
{
	UProperty* P = *(*(TFieldIterator<UProperty>*)It);
	P->ImportText( (const TCHAR*)Text, (BYTE*)Obj + P->Offset, 0 );
}
static void* NetConnectionDriver( void* Conn ) { return ((UNetConnection*)Conn)->Driver; }
static void* NetConnectionPackageMap( void* Conn ) { return ((UNetConnection*)Conn)->PackageMap; }
static void NetConnectionFlushNet( void* Conn ) { ((UNetConnection*)Conn)->FlushNet(); }
static void NetConnectionSendPackageMap( void* Conn ) { ((UNetConnection*)Conn)->SendPackageMap(); }
static void NetConnectionChannelClose( void* Conn, int32_t I ) { ((UNetConnection*)Conn)->Channels[I]->Close(); }
static int32_t NetConnectionRemoteVersion( void* Conn ) { return ((UNetConnection*)Conn)->RemoteVersion; }
static void NetConnectionSetRemoteVersion( void* Conn, int32_t V ) { ((UNetConnection*)Conn)->RemoteVersion = V; }
static void NetConnectionSetStateClosed( void* Conn ) { ((UNetConnection*)Conn)->State = USOCK_Closed; }
static void NetConnectionSetNegotiatedVer( void* Conn, int32_t V ) { ((UNetConnection*)Conn)->NegotiatedVer = V; }
static uint32_t NetConnectionChallenge( void* Conn ) { return ((UNetConnection*)Conn)->Challenge; }
static void NetConnectionSetChallenge( void* Conn, uint32_t V ) { ((UNetConnection*)Conn)->Challenge = V; }
static void NetConnectionSetUserFlags( void* Conn, int32_t V ) { ((UNetConnection*)Conn)->UserFlags = V; }
static void NetConnectionSetCurrentNetSpeed( void* Conn, int32_t V ) { ((UNetConnection*)Conn)->CurrentNetSpeed = V; }
static int32_t NetConnectionCurrentNetSpeed( void* Conn ) { return ((UNetConnection*)Conn)->CurrentNetSpeed; }
static void NetConnectionSetRequestUrl( void* Conn, const uint32_t* Text ) { ((UNetConnection*)Conn)->RequestURL = (const TCHAR*)Text; }
static const uint32_t* NetConnectionRequestUrlText( void* Conn ) { return (const uint32_t*)*((UNetConnection*)Conn)->RequestURL; }
static void* NetConnectionActor( void* Conn ) { return ((UNetConnection*)Conn)->Actor; }
static const uint32_t* NetConnectionActorPlayerName( void* Conn ) { return (const uint32_t*)*((UNetConnection*)Conn)->Actor->PlayerReplicationInfo->PlayerName; }
static void* NetConnectionLowLevelRemoteAddress( void* Conn ) { return new FString( ((UNetConnection*)Conn)->LowLevelGetRemoteAddress() ); }
static void NetPackageMapCopy( void* Pm, void* Master ) { ((UPackageMap*)Pm)->Copy( (UPackageMap*)Master ); }
static void NetPackageMapCompute( void* Pm ) { ((UPackageMap*)Pm)->Compute(); }
static int32_t NetPackageMapListNum( void* Pm ) { return ((UPackageMap*)Pm)->List.Num(); }
static int32_t NetPackageInfoGuidEq( void* Pm, int32_t I, const uint32_t* Guid ) { return ((UPackageMap*)Pm)->List( I ).Guid == *(const FGuid*)Guid ? 1 : 0; }
static int32_t* NetPackageInfoRemoteGeneration( void* Pm, int32_t I ) { return &((UPackageMap*)Pm)->List( I ).RemoteGeneration; }
static const uint32_t* NetPackageInfoUrlText( void* Pm, int32_t I ) { return (const uint32_t*)*((UPackageMap*)Pm)->List( I ).URL; }
static void NetPackageMapListRemove( void* Pm, int32_t I ) { ((UPackageMap*)Pm)->List.Remove( I ); }
static void* NetChannelConnection( void* Ch ) { return ((UChannel*)Ch)->Connection; }
static int32_t NetChannelChType( void* Ch ) { return ((UChannel*)Ch)->ChType; }
static int32_t NetChannelChIndex( void* Ch ) { return ((UChannel*)Ch)->ChIndex; }

static Hp2LevelNetServices MakeNetServices( void )
{
	Hp2LevelNetServices s;
	memset( &s, 0, sizeof(s) );
	s.debugf_log = NetDebugfLog;
	s.debugf_dev_net = NetDebugfDevNet;
	s.debugf_net_come_go = NetDebugfNetComeGo;
	s.output_device_log = NetOutputDeviceLog;
	s.output_device_logf = NetOutputDeviceLogf;
	s.connection_log = NetConnectionLog;
	s.check_failed = NetCheckFailed;
	s.app_errorf = NetAppErrorf;
	s.localize_error_assign = NetLocalizeErrorAssign;
	s.uobject_get_name = NetUObjectGetName;
	s.uobject_get_full_name = NetUObjectGetFullName;
	s.uobject_get_outer = NetUObjectGetOuter;
	s.level_get_linker = NetLevelGetLinker;
	s.app_timestamp = NetAppTimestamp;
	s.app_cycles = NetAppCycles;
	s.app_atoi = NetAppAtoi;
	s.app_strchr = NetAppStrchr;
	s.fstring_new = NetFStringNew;
	s.fstring_delete = NetFStringDelete;
	s.fstring_text = NetFStringText;
	s.fstring_assign = NetFStringAssign;
	s.fstring_append_text = NetFStringAppendText;
	s.fstring_ne_text = NetFStringNeText;
	s.fstring_right = NetFStringRight;
	s.fstring_left = NetFStringLeft;
	s.fstring_mid = NetFStringMid;
	s.fstring_add = NetFStringAdd;
	s.fstring_caps_in_str = NetFStringCapsInStr;
	s.furl_new = NetFURLNew;
	s.furl_new_travel = NetFURLNewTravel;
	s.furl_delete = NetFURLDelete;
	s.furl_map = NetFURLMap;
	s.parse_command = NetParseCommand;
	s.parse_token_fstring = NetParseTokenFString;
	s.parse_token_buf = NetParseTokenBuf;
	s.parse_int = NetParseInt;
	s.parse_text = NetParseText;
	s.parse_guid = NetParseGuid;
	s.level_exec_net = NetLevelExecNet;
	s.level_exec_demo = NetLevelExecDemo;
	s.static_load_class = NetStaticLoadClass;
	s.construct_net_driver = NetConstructNetDriver;
	s.static_construct_net_driver = NetStaticConstructNetDriver;
	s.netdriver_init_listen = NetNetDriverInitListen;
	s.netdriver_delete = NetNetDriverDelete;
	s.netdriver_server_connection = NetNetDriverServerConnection;
	s.netdriver_server_travel_pause = NetNetDriverServerTravelPause;
	s.netdriver_max_client_rate = NetNetDriverMaxClientRate;
	s.netdriver_master_map = NetNetDriverMasterMap;
	s.cast_game_engine = NetCastGameEngine;
	s.game_engine_gpending_level = NetGameEngineGPendingLevel;
	s.game_engine_set_gpending_level = NetGameEngineSetGPendingLevel;
	s.game_engine_cancel_pending = NetGameEngineCancelPending;
	s.game_build_server_master_map = NetGameBuildServerMasterMap;
	s.game_engine_server_actors_num = NetGameEngineServerActorsNum;
	s.game_engine_server_actor_text = NetGameEngineServerActorText;
	s.demo_play_pending_level_new = NetDemoPlayPendingLevelNew;
	s.pending_level_demo_rec_driver = NetPendingLevelDemoRecDriver;
	s.pending_level_error_text = NetPendingLevelErrorText;
	s.pending_level_delete = NetPendingLevelDelete;
	s.engine_client = NetEngineClient;
	s.engine_client_viewports_num = NetEngineClientViewportsNum;
	s.engine_client_viewport = NetEngineClientViewport;
	s.engine_set_client_travel = NetEngineSetClientTravel;
	s.engine_challenge_response = NetEngineChallengeResponse;
	s.level_spawn_actor = NetLevelSpawnActor;
	s.level_url = NetLevelUrl;
	s.level_get_level_info = NetLevelGetLevelInfo;
	s.spawn_play_actor = NetSpawnPlayActor;
	s.level_welcome_player = NetLevelWelcomePlayer;
	s.level_info_game = NetLevelInfoGame;
	s.level_info_next_url = NetLevelInfoNextUrl;
	s.level_info_b_lone_player = NetLevelInfoBLonePlayer;
	s.level_info_set_next_switch_countdown = NetLevelInfoSetNextSwitchCountdown;
	s.level_info_set_net_mode = NetLevelInfoSetNetMode;
	s.game_info_b_world_log = NetGameInfoBWorldLog;
	s.game_pre_login = NetGamePreLogin;
	s.actor_get_class = NetActorGetClass;
	s.tfield_iter_new = NetTFieldIterNew;
	s.tfield_iter_valid = NetTFieldIterValid;
	s.tfield_iter_next = NetTFieldIterNext;
	s.tfield_iter_delete = NetTFieldIterDelete;
	s.uproperty_name_stricmp = NetUPropertyNameStricmp;
	s.uproperty_is_config = NetUPropertyIsConfig;
	s.uproperty_import_text = NetUPropertyImportText;
	s.connection_driver = NetConnectionDriver;
	s.connection_package_map = NetConnectionPackageMap;
	s.connection_flush_net = NetConnectionFlushNet;
	s.connection_send_package_map = NetConnectionSendPackageMap;
	s.connection_channel_close = NetConnectionChannelClose;
	s.connection_remote_version = NetConnectionRemoteVersion;
	s.connection_set_remote_version = NetConnectionSetRemoteVersion;
	s.connection_set_state_closed = NetConnectionSetStateClosed;
	s.connection_set_negotiated_ver = NetConnectionSetNegotiatedVer;
	s.connection_challenge = NetConnectionChallenge;
	s.connection_set_challenge = NetConnectionSetChallenge;
	s.connection_set_user_flags = NetConnectionSetUserFlags;
	s.connection_set_current_net_speed = NetConnectionSetCurrentNetSpeed;
	s.connection_current_net_speed = NetConnectionCurrentNetSpeed;
	s.connection_set_request_url = NetConnectionSetRequestUrl;
	s.connection_request_url_text = NetConnectionRequestUrlText;
	s.connection_actor = NetConnectionActor;
	s.connection_actor_player_name = NetConnectionActorPlayerName;
	s.connection_lowlevel_remote_address = NetConnectionLowLevelRemoteAddress;
	s.package_map_copy = NetPackageMapCopy;
	s.package_map_compute = NetPackageMapCompute;
	s.package_map_list_num = NetPackageMapListNum;
	s.package_info_guid_eq = NetPackageInfoGuidEq;
	s.package_info_remote_generation = NetPackageInfoRemoteGeneration;
	s.package_info_url_text = NetPackageInfoUrlText;
	s.package_map_list_remove = NetPackageMapListRemove;
	s.channel_connection = NetChannelConnection;
	s.channel_ch_type = NetChannelChType;
	s.channel_ch_index = NetChannelChIndex;
	return s;
}

const Hp2LevelNetServices GHp2LevelNetServices = MakeNetServices();
