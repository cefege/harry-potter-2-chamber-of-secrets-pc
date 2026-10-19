/*=============================================================================
	UnActor.cpp: AActor implementation
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "UnMesh.h"
#include "Hp2ActorServices.h"
#include "Hp2ActorRepServices.h"
/*-----------------------------------------------------------------------------
	AActor object implementations.
-----------------------------------------------------------------------------*/
static UStruct* GetVectorStructMetadata()
{
	UStruct* Struct = FindObject<UStruct>(UObject::StaticClass(),TEXT("Vector"));
	if( !Struct )
	{
		Struct = new(UObject::StaticClass(),TEXT("Vector"),RF_Public)UStruct(NULL);
		Struct->SetPropertiesSize(sizeof(FVector));
		new(Struct,TEXT("X"),RF_Public)UFloatProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FVector,X)),TEXT(""),0);
		new(Struct,TEXT("Y"),RF_Public)UFloatProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FVector,Y)),TEXT(""),0);
		new(Struct,TEXT("Z"),RF_Public)UFloatProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FVector,Z)),TEXT(""),0);
		FArchive ArDummy;
		Struct->Link(ArDummy,0);
	}
	return Struct;
}
static UStruct* GetColorStructMetadata()
{
	UStruct* Struct = FindObject<UStruct>(UObject::StaticClass(),TEXT("Color"));
	if( !Struct )
	{
		Struct = new(UObject::StaticClass(),TEXT("Color"),RF_Public)UStruct(NULL);
		Struct->SetPropertiesSize(sizeof(FColor));
		new(Struct,TEXT("R"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,R)),TEXT(""),0);
		new(Struct,TEXT("G"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,G)),TEXT(""),0);
		new(Struct,TEXT("B"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,B)),TEXT(""),0);
		new(Struct,TEXT("A"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,A)),TEXT(""),0);
		FArchive ArDummy;
		Struct->Link(ArDummy,0);
	}
	return Struct;
}


void AActor::StaticConstructor()
{
	guard(AActor::StaticConstructor);

	const INT CollisionBoolOffset = static_cast<INT>(__builtin_offsetof(AActor, LightType) - sizeof(BITFIELD));
	UBoolProperty* AlignBottom = new(GetClass(),TEXT("bAlignBottom"),RF_Public)
		UBoolProperty(EC_CppProperty,CollisionBoolOffset,TEXT("Collision"),CPF_Edit);
	AlignBottom->BitMask = 64;
	UBoolProperty* AlignBottomAlways = new(GetClass(),TEXT("bAlignBottomAlways"),RF_Public)
		UBoolProperty(EC_CppProperty,CollisionBoolOffset,TEXT("Collision"),CPF_Edit);
	AlignBottomAlways->BitMask = 128;
	UBoolProperty* BlockCamera = new(GetClass(),TEXT("bBlockCamera"),RF_Public)
		UBoolProperty(EC_CppProperty,CollisionBoolOffset,TEXT("Collision"),CPF_Edit);
	BlockCamera->BitMask = 256;

	unguard;
}

void AInterpolationManager::StaticConstructor()
{
	guard(AInterpolationManager::StaticConstructor);

	UStruct* VectorStruct = GetVectorStructMetadata();
	UStruct* ColorStruct = GetColorStructMetadata();
#define REGISTER_INTERPOLATION_PROPERTY(PropertyType,Member) \
	new(GetClass(),TEXT(#Member),RF_Public)PropertyType(CPP_PROPERTY(Member),TEXT(""),0)
#define REGISTER_INTERPOLATION_OBJECT(Member,ObjectClass) \
	new(GetClass(),TEXT(#Member),RF_Public)UObjectProperty(CPP_PROPERTY(Member),TEXT(""),0,ObjectClass)
#define REGISTER_INTERPOLATION_STRUCT(Member,StructMetadata) \
	new(GetClass(),TEXT(#Member),RF_Public)UStructProperty(CPP_PROPERTY(Member),TEXT(""),0,StructMetadata)

	REGISTER_INTERPOLATION_OBJECT(Dest,AInterpolationPoint::StaticClass());
	REGISTER_INTERPOLATION_OBJECT(Last,AInterpolationPoint::StaticClass());
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,PhysAlpha);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,PhysRate);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,RemainingPause);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,StartPause);
	REGISTER_INTERPOLATION_PROPERTY(UIntProperty,PauseNum);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,OldGameSpeed);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,OldFOVModifier);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,OldFlashScale);
	REGISTER_INTERPOLATION_STRUCT(OldFlashFog,VectorStruct);
	REGISTER_INTERPOLATION_STRUCT(OldFogColor,ColorStruct);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,OldFogStart);
	REGISTER_INTERPOLATION_PROPERTY(UFloatProperty,OldFogEnd);
	REGISTER_INTERPOLATION_STRUCT(TurnRateX,VectorStruct);
	REGISTER_INTERPOLATION_STRUCT(TurnRateZ,VectorStruct);
	REGISTER_INTERPOLATION_STRUCT(OldDesiredX,VectorStruct);
	REGISTER_INTERPOLATION_STRUCT(OldDesiredZ,VectorStruct);

	const INT InstantMoveOffset = static_cast<INT>(sizeof(AInterpolationManager) - sizeof(BITFIELD));
	UBoolProperty* InstantMove = new(GetClass(),TEXT("bInstantMove"),RF_Public)
		UBoolProperty(EC_CppProperty,InstantMoveOffset,TEXT(""),0);
	InstantMove->BitMask = 1;

#undef REGISTER_INTERPOLATION_STRUCT
#undef REGISTER_INTERPOLATION_OBJECT
#undef REGISTER_INTERPOLATION_PROPERTY

	unguard;
}

void APatrolPoint::StaticConstructor()
{
	guard(APatrolPoint::StaticConstructor);

	UStruct* VectorStruct = GetVectorStructMetadata();
#define REGISTER_PATROL_PROPERTY(PropertyType,Member,Category,Flags) \
	new(GetClass(),TEXT(#Member),RF_Public)PropertyType(CPP_PROPERTY(Member),Category,Flags)
#define REGISTER_PATROL_OBJECT(Member,ObjectClass,Category,Flags) \
	new(GetClass(),TEXT(#Member),RF_Public)UObjectProperty(CPP_PROPERTY(Member),Category,Flags,ObjectClass)
#define REGISTER_PATROL_STRUCT(Member,StructMetadata,Category,Flags) \
	new(GetClass(),TEXT(#Member),RF_Public)UStructProperty(CPP_PROPERTY(Member),Category,Flags,StructMetadata)
#define REGISTER_PATROL_BOOL(Member,StorageOffset,Mask,Category,Flags) \
	{ UBoolProperty* Property = new(GetClass(),TEXT(#Member),RF_Public)UBoolProperty(EC_CppProperty,StorageOffset,Category,Flags); Property->BitMask = Mask; }

	REGISTER_PATROL_PROPERTY(UNameProperty,NextPatrol_ObjectName,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UFloatProperty,PauseTime,TEXT("PatrolPoint"),CPF_Edit);
	const INT LookBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, lookDir) - sizeof(BITFIELD));
	REGISTER_PATROL_BOOL(bUseLookDir,LookBoolOffset,1,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_STRUCT(lookDir,VectorStruct,TEXT(""),0);
	REGISTER_PATROL_PROPERTY(UNameProperty,PatrolAnim,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UNameProperty,PauseAnim,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_OBJECT(PatrolSound,USound::StaticClass(),TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UByteProperty,numAnims,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UIntProperty,AnimCount,TEXT(""),0);
	const INT LeadBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, PrevPatrolPoint) - sizeof(BITFIELD));
	REGISTER_PATROL_BOOL(bLeadActorWaitPoint,LeadBoolOffset,1,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_OBJECT(PrevPatrolPoint,GetClass(),TEXT(""),0);
	REGISTER_PATROL_OBJECT(NextPatrolPoint,GetClass(),TEXT(""),0);
	REGISTER_PATROL_PROPERTY(UNameProperty,EventToSend,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UNameProperty,ActionKeyword,TEXT("PatrolPoint"),CPF_Edit);
	const INT ChainBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, PatrolPointLinkTag) - sizeof(BITFIELD));
	REGISTER_PATROL_BOOL(bDestroyPawn,ChainBoolOffset,1,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_BOOL(bUseOrientationForSplineTan,ChainBoolOffset,2,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_BOOL(bStartOfUnlinkedChain,ChainBoolOffset,4,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UNameProperty,PatrolPointLinkTag,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_STRUCT(vFraySplineTangent,VectorStruct,TEXT(""),0);
	const INT SplineBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, fTanLenIn) - sizeof(BITFIELD));
	REGISTER_PATROL_BOOL(bHasSplineInfo,SplineBoolOffset,1,TEXT(""),0);
	REGISTER_PATROL_PROPERTY(UFloatProperty,fTanLenIn,TEXT(""),0);
	REGISTER_PATROL_PROPERTY(UFloatProperty,fTanLenOut,TEXT(""),0);
	REGISTER_PATROL_PROPERTY(UFloatProperty,fJumpHorizSpeed,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UFloatProperty,fJumpVertSpeed,TEXT("PatrolPoint"),CPF_Edit);
	REGISTER_PATROL_PROPERTY(UFloatProperty,fJumpAnimMultiplier,TEXT("PatrolPoint"),CPF_Edit);
	const INT BossBoolOffset = static_cast<INT>(sizeof(APatrolPoint) - sizeof(BITFIELD));
	REGISTER_PATROL_BOOL(bStopBossEncounter,BossBoolOffset,1,TEXT("PatrolPoint"),CPF_Edit);

#undef REGISTER_PATROL_BOOL
#undef REGISTER_PATROL_STRUCT
#undef REGISTER_PATROL_OBJECT
#undef REGISTER_PATROL_PROPERTY

	unguard;
}


IMPLEMENT_CLASS(AActor);
IMPLEMENT_CLASS(ALight);
#if 1 //U2Ed
IMPLEMENT_CLASS(AClipMarker);
#endif
IMPLEMENT_CLASS(AWeapon);
IMPLEMENT_CLASS(ALevelInfo);
IMPLEMENT_CLASS(AGameInfo);
IMPLEMENT_CLASS(ACamera);
IMPLEMENT_CLASS(AZoneInfo);
IMPLEMENT_CLASS(ASkyZoneInfo);
IMPLEMENT_CLASS(APathNode);
IMPLEMENT_CLASS(ANavigationPoint);
IMPLEMENT_CLASS(APatrolPoint);
IMPLEMENT_CLASS(AScout);
IMPLEMENT_CLASS(AInterpolationPoint);
IMPLEMENT_CLASS(ADecoration);
IMPLEMENT_CLASS(AProjectile);
IMPLEMENT_CLASS(AWarpZoneInfo);
IMPLEMENT_CLASS(ATeleporter);
IMPLEMENT_CLASS(APlayerStart);
IMPLEMENT_CLASS(AKeypoint);
IMPLEMENT_CLASS(AInventory);
IMPLEMENT_CLASS(AInventorySpot);
IMPLEMENT_CLASS(ATriggers);
IMPLEMENT_CLASS(ATrigger);
IMPLEMENT_CLASS(ATriggerMarker);
IMPLEMENT_CLASS(AButtonMarker);
IMPLEMENT_CLASS(AWarpZoneMarker);
IMPLEMENT_CLASS(AHUD);
IMPLEMENT_CLASS(AMenu);
IMPLEMENT_CLASS(ASavedMove);
IMPLEMENT_CLASS(ACarcass);
IMPLEMENT_CLASS(ALiftCenter);
IMPLEMENT_CLASS(ALiftExit);
IMPLEMENT_CLASS(AInfo);
IMPLEMENT_CLASS(AReplicationInfo);
IMPLEMENT_CLASS(APlayerReplicationInfo);
IMPLEMENT_CLASS(AInternetInfo);
IMPLEMENT_CLASS(AStatLog);
IMPLEMENT_CLASS(AStatLogFile);
IMPLEMENT_CLASS(AGameReplicationInfo);
IMPLEMENT_CLASS(ULevelSummary);
IMPLEMENT_CLASS(ALocationID);
IMPLEMENT_CLASS(ADecal);
IMPLEMENT_CLASS(ASpawnNotify);
IMPLEMENT_CLASS(AAmmo);
IMPLEMENT_CLASS(APickup); 
IMPLEMENT_CLASS(AInterpolationManager);
IMPLEMENT_CLASS(USoundContainer);
//IMPLEMENT_CLASS(UFootSoundSet);
IMPLEMENT_CLASS(UImpactSoundSet);
IMPLEMENT_CLASS(UGameSaveInfo);



/*-----------------------------------------------------------------------------
	Replication.
-----------------------------------------------------------------------------*/
INT* AActor::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AActor::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_actor_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr );
	unguard;
}

FLOAT AActor::UpdateFrequency(AActor *Viewer, FVector &ViewDir, FVector &ViewPos)

{
	guard(AActor::UpdateFrequency);
	return hp2_rs_engine_actor_rep_actor_update_frequency( &GHp2ActorRepServices, this, &ViewDir, &ViewPos );
	unguard;
}

INT* APawn::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APawn::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_pawn_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr, NumReps );
	unguard;
}

INT* APlayerPawn::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APlayerPawn::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_player_pawn_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr, NumReps );
	unguard;
}
INT* AMover::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AMover::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_mover_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr );
	unguard;
}

INT* AZoneInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AZoneInfo::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_zone_info_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr );
	unguard;
}

INT* ALevelInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(ALevelInfo::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_level_info_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr );
	unguard;
}

INT* APlayerReplicationInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APlayerReplicationInfo::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_player_replication_info_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr );
	unguard;
}
INT* AGameReplicationInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AGameReplicationInfo::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_game_replication_info_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr );
	unguard;
}
INT* AInventory::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AInventory::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_inventory_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr, NumReps );
	unguard;
}
UBOOL AInventory::ShouldDoScriptReplication()
{
	return hp2_rs_engine_actor_rep_inventory_should_do_script_replication( this );
}


UBOOL AInventory::NoVariablesToReplicate(AActor *OldVer)
{
	return hp2_rs_engine_actor_rep_inventory_no_variables_to_replicate( this, OldVer );
}

FLOAT AInventory::UpdateFrequency(AActor *Viewer, FVector &ViewDir, FVector &ViewPos)
{
	guard(AInventory::UpdateFrequency);
	return hp2_rs_engine_actor_rep_inventory_update_frequency( &GHp2ActorRepServices, this, Viewer );
	unguard;
}

FLOAT ACarcass::UpdateFrequency(AActor *Viewer, FVector &ViewDir, FVector &ViewPos)
{
	guard(ACarcass::UpdateFrequency);
	return hp2_rs_engine_actor_carcass_update_frequency( this );
	unguard;
}

INT* AWeapon::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AWeapon::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_weapon_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr, NumReps );
	unguard;
}

INT* AAmmo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AAmmo::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_ammo_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr, NumReps );
	unguard;
}

INT* APickup::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APickup::GetOptimizedRepList);
	return hp2_rs_engine_actor_rep_pickup_get_optimized_rep_list( &GHp2ActorRepServices, this, Recent, Map, Ptr, NumReps );
	unguard;
}
/*-----------------------------------------------------------------------------
	AActor networking implementation.
-----------------------------------------------------------------------------*/

//
// Static variables for networking.
//
static FVector   SavedLocation;
static FRotator  SavedRotation;
static AActor*   SavedBase;
static DWORD     SavedCollision;
static FLOAT	 SavedRadius;
static FLOAT     SavedHeight;
static FPlane    SavedSimAnim;
static FVector	 SavedSimInterpolate;

//
// Skins.
//
UTexture* AActor::GetSkin( INT Index )
{
	return (UTexture*)hp2_rs_engine_actor_get_skin( this, Index );
}

//
// Net priority.
//
FLOAT AActor::GetNetPriority( AActor* Sent, FLOAT Time, FLOAT Lag )
{
	guardSlow(AActor::GetNetPriority);
	return hp2_rs_engine_actor_get_net_priority( this, Time );
	unguardSlow;
}

//
// Always called immediately before properties are received from the remote.
//
void AActor::PreNetReceive()
{
	guard(AActor::PreNetReceive);
	hp2_rs_engine_actor_pre_net_receive( &GHp2ActorServices, this );
	unguard;
}

//
// Always called immediately after properties are received from the remote.
//
void AActor::PostNetReceive()
{
	guard(AActor::PostNetReceive);
	hp2_rs_engine_actor_post_net_receive( &GHp2ActorServices, this );
	unguard;
}

/*-----------------------------------------------------------------------------
	APlayerPawn implementation.
-----------------------------------------------------------------------------*/

//
// Set the player.
//
void APlayerPawn::SetPlayer( UPlayer* InPlayer )
{
	guard(APlayerPawn::SetPlayer);
	hp2_rs_engine_actor_set_player( &GHp2ActorServices, this, InPlayer );
	unguard;
}

bool APlayerPawn::ClearScreen()
{
	return false;
}
bool APlayerPawn::RecomputeLighting()
{
	return false;
}
bool APlayerPawn::CanSee( const AActor* Actor )
{
	return false;
}
INT APlayerPawn::GetViewZone( INT iViewZone, const UModel* Model )
{
	return iViewZone;
}
bool APlayerPawn::IsZoneVisible( INT iZone )
{
	return true;
}
bool APlayerPawn::IsSurfVisible( const FBspNode* Node, INT iZone, const FBspSurf* Poly )
{
	return true;
}
bool APlayerPawn::IsActorVisible( const AActor* Actor )
{
	return true;
}

/*-----------------------------------------------------------------------------
	AZoneInfo.
-----------------------------------------------------------------------------*/

void AZoneInfo::PostEditChange()
{
	guard(AZoneInfo::PostEditChange);
	Super::PostEditChange();
	hp2_rs_engine_actor_zone_info_post_edit_change( &GHp2ActorServices );
	unguard;
}

/*-----------------------------------------------------------------------------
	AActor.
-----------------------------------------------------------------------------*/

void AActor::Destroy()
{
	guard(AActor::Destroy);
	hp2_rs_engine_actor_destroy( &GHp2ActorServices, this );
	unguard;
}

void AActor::PostLoad()
{
	guard(AActor::PostLoad);
	hp2_rs_engine_actor_post_load( &GHp2ActorServices, this );
	unguard;
}

void AActor::Spawned()
{
	hp2_rs_engine_actor_spawned( &GHp2ActorServices, this );
}

void AActor::ProcessEvent( UFunction* Function, void* Parms, void* Result )
{
	guardSlow(AActor::ProcessEvent);
	hp2_rs_engine_actor_process_event( &GHp2ActorServices, this, Function, Parms, Result );
	unguardSlow;
}

void AActor::PostEditChange()
{
	guard(AActor::PostEditChange);
	Super::PostEditChange();
	hp2_rs_engine_actor_post_edit_change( &GHp2ActorServices, this );
	unguard;
}

//
// Set the actor's collision properties.
//
void AActor::SetCollision
(
	UBOOL NewCollideActors,
	UBOOL NewBlockActors,
	UBOOL NewBlockPlayers
)
{
	guard(AActor::SetCollision);
	hp2_rs_engine_actor_set_collision( &GHp2ActorServices, this, NewCollideActors, NewBlockActors, NewBlockPlayers );
	unguard;
}

//
// Set collision size.
//
void AActor::SetCollisionSize( FLOAT NewRadius, FLOAT NewHeight )
{
	guard(AActor::SetCollisionSize);
	hp2_rs_engine_actor_set_collision_size( &GHp2ActorServices, this, NewRadius, NewHeight );
	unguard;
}

//
// Return whether this actor overlaps another.
//
UBOOL AActor::IsOverlapping( const AActor* Other ) const
{
	UBOOL Result;
	guardSlow(AActor::IsOverlapping);
	checkSlow(Other!=NULL);
	Result = hp2_rs_engine_actor_is_overlapping( &GHp2ActorServices, this, Other );
	unguardSlow;
	return Result;
}

//
// Return visiblity box.
//
FBox AActor::GetVisibilityBox()
{
	guard(AActor::GetVisibilityBox);
	FBox Result(0);
	hp2_rs_engine_actor_get_visibility_box( this, &Result );
	return Result;
	unguard;
}

/*-----------------------------------------------------------------------------
	Actor touch minions.
-----------------------------------------------------------------------------*/
//
// Note that TouchActor has begun touching Actor.
//
// If an actor's touch list overflows, neither actor receives the
// touch messages, as if they are not touching.
//
// This routine is reflexive.
//
// Handles the case of the first-notified actor changing its touch status.
//
void AActor::BeginTouch( AActor* Other )
{
	guard(AActor::BeginTouch);
	hp2_rs_engine_actor_begin_touch( &GHp2ActorServices, this, Other );
	unguard;
}

//
// Note that TouchActor is no longer touching Actor.
//
// If NoNotifyActor is specified, Actor is not notified but
// TouchActor is (this happens during actor destruction).
//
void AActor::EndTouch( AActor* Other, UBOOL NoNotifySelf )
{
	guard(AActor::EndTouch);
	check(Other!=this);
	hp2_rs_engine_actor_end_touch( &GHp2ActorServices, this, Other, NoNotifySelf );
	unguard;
}

/*-----------------------------------------------------------------------------
	AActor member functions.
-----------------------------------------------------------------------------*/

//
// Destroy the actor.
//
void AActor::Serialize( FArchive& Ar )
{
	guard(AActor::Serialize);
	Super::Serialize( Ar );
	unguard;
}

// Serialize Persistent actor properties
void AActor::SerializePersistence( FArchive& Ar )
{
	guard(UObject::SerializePersistence);
	hp2_rs_engine_actor_serialize_persistence( &GHp2ActorServices, this, &Ar );
/*
	// Special info.
	if( (!Ar.IsLoading() && !Ar.IsSaving()) || Ar.IsTrans() )
		Ar << GetName() << Outer << TheClass;
	if( !Ar.IsLoading() && !Ar.IsSaving() )
		Ar << _Linker;

	// **** Serialize FRAME STATE ****
	// Execution stack.
	//!!how does the stack work in conjunction with transaction tracking?
	guard(SerializeStack);
	
	FStateFrame* TheStateFrame = GetStateFrame();
	if( !Ar.IsTrans() )
	{
		if( GetFlags() & RF_HasStack )
		{
			if( !TheStateFrame )
				TheStateFrame = new(TEXT("ObjectTheStateFrame")) FStateFrame( this );
			Ar << TheStateFrame->Node << TheStateFrame->StateNode;
			Ar << TheStateFrame->ProbeMask;
			Ar << TheStateFrame->LatentAction;
			if( TheStateFrame->Node )
			{
				Ar.Preload( TheStateFrame->Node );
				if( Ar.IsSaving() && TheStateFrame->Code )
					check(TheStateFrame->Code>=&TheStateFrame->Node->Script(0) && TheStateFrame->Code<&TheStateFrame->Node->Script(TheStateFrame->Node->Script.Num()));
				INT Offset = TheStateFrame->Code ? TheStateFrame->Code - &TheStateFrame->Node->Script(0) : INDEX_NONE;
				Ar << AR_INDEX(Offset);
				if( Offset!=INDEX_NONE )
					if( Offset<0 || Offset>=TheStateFrame->Node->Script.Num() )
						appErrorf( TEXT("%s: Offset mismatch: %i %i"), GetFullName(), Offset, TheStateFrame->Node->Script.Num() );
				TheStateFrame->Code = Offset!=INDEX_NONE ? &TheStateFrame->Node->Script(Offset) : NULL;
			}
			else TheStateFrame->Code = NULL;
		}
		else if( TheStateFrame )
		{
			delete TheStateFrame;
			TheStateFrame = NULL;
		}
	}
	unguard;
*/
	unguardobj;
}


/*-----------------------------------------------------------------------------
	Relations.
-----------------------------------------------------------------------------*/

//
// Change the actor's owner.
//
void AActor::SetOwner( AActor *NewOwner )
{
	guard(AActor::SetOwner);
	
	// Sets this actor's parent to the specified actor.
	hp2_rs_engine_actor_set_owner( &GHp2ActorServices, this, NewOwner );
	unguard;
}

//
// Change the actor's base.
//
void AActor::SetBase( AActor* NewBase, int bNotifyActor )
{
	guard(AActor::SetBase);
	hp2_rs_engine_actor_set_base( &GHp2ActorServices, this, NewBase, bNotifyActor );
	unguard;
}

//
// Get the actor's primitive.
//
UPrimitive* AActor::GetPrimitive() const
{
	UPrimitive* Result;
	guardSlow(AActor::GetPrimitive);
	Result = (UPrimitive*)hp2_rs_engine_actor_get_primitive( &GHp2ActorServices, this );
	unguardSlow;
	return Result;
}

FVector AActor::GetCylinderExtent() const
{
	// This is handled here for all collision types,
	// rather than using GetPrimitive()->GetCollisionExtent(),
	// so that it can be called on a default object using non-virtual function.
	FVector Result;
	guardSlow(AActor::GetCylinderExtent);
	hp2_rs_engine_actor_get_cylinder_extent( &GHp2ActorServices, (void*)this, &Result );
	unguardSlow;
	return Result;
}

FCoords AActor::GetRenderBoundingBox( UBOOL Exact )
{
	FCoords Result;
	hp2_rs_engine_actor_get_render_bounding_box( &GHp2ActorServices, this, Exact, &Result );
	return Result;
}

/*-----------------------------------------------------------------------------
	Mesh Support.
-----------------------------------------------------------------------------*/
// Taken from Undying ...

static UBOOL PointInsideQuad( const FVector& Point, const FVector Verts[], const FVector& Normal )
{
	FLOAT PrevDot = 0;
	for( INT i=0, j = 3; i < 4; j = i, ++i )
	{
		const FVector& V0 = Verts[j];
		const FVector& V1 = Verts[i];
		FVector ClipNorm = Normal ^ (V1 - V0);
		FPlane ClipPlane( V0, ClipNorm );

		FLOAT Dot = ClipPlane.PlaneDot( Point );

		if( Dot * PrevDot < 0.0f )
		{
			return false;
		}
		PrevDot = Dot;
	}

	return true;
}


// TraceTexture
// Traces a line and returns a UTexture from the struck surface
void AActor::execTraceTexture(FFrame& Stack, RESULT_DECL)
{
	guardSlow(AActor::execTraceTexture);

	P_GET_VECTOR(TraceEnd);
	P_GET_VECTOR(TraceStart);
	P_GET_INT_REF(Flags);
	P_GET_UBOOL_OPTX(bTraceDecals, 0);
	//P_GET_VECTOR_REF(ScrollDir);	
	P_FINISH;
	*(UTexture **)Result = (UTexture*)hp2_rs_engine_actor_trace_texture( &GHp2ActorServices, this, &TraceEnd, &TraceStart, Flags, bTraceDecals );
	unguardexecSlow;
}



/*-----------------------------------------------------------------------------
	Special editor support.
-----------------------------------------------------------------------------*/
AActor* AActor::GetHitActor()
{
	return (AActor*)hp2_rs_engine_actor_get_hit_actor( this );
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/
