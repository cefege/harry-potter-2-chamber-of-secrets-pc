/*=============================================================================
	UnActor.cpp: AActor implementation
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "UnMesh.h"

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

UBOOL NEQ(BYTE A,BYTE B,UPackageMap* Map) {return A!=B;}
UBOOL NEQ(INT A,INT B,UPackageMap* Map) {return A!=B;}
UBOOL NEQ(BITFIELD A,BITFIELD B,UPackageMap* Map) {return A!=B;}
UBOOL NEQ(FLOAT& A,FLOAT& B,UPackageMap* Map) {return *(INT*)&A!=*(INT*)&B;}
UBOOL NEQ(FVector& A,FVector& B,UPackageMap* Map) {return ((INT*)&A)[0]!=((INT*)&B)[0] || ((INT*)&A)[1]!=((INT*)&B)[1] || ((INT*)&A)[2]!=((INT*)&B)[2];}
UBOOL NEQ(FRotator& A,FRotator& B,UPackageMap* Map) {return A.Pitch!=B.Pitch || A.Yaw!=B.Yaw || A.Roll!=B.Roll;}
UBOOL NEQ(UObject* A,UObject* B,UPackageMap* Map) {return (Map->CanSerializeObject(A)?A:NULL)!=B;}
UBOOL NEQ(FName& A,FName B,UPackageMap* Map) {return *(INT*)&A!=*(INT*)&B;}
//UBOOL NEQ(FColor& A,FColor& B,UPackageMap* Map) {return *(INT*)&A!=*(INT*)&B;} //!! Alignment
UBOOL NEQ(FColor& A,FColor& B,UPackageMap* Map) {return A.R!=B.R || A.G!=B.G || A.B!=B.B || A.A!=B.A; }
UBOOL NEQ(FPlane& A,FPlane& B,UPackageMap* Map) {return
((INT*)&A)[0]!=((INT*)&B)[0] || ((INT*)&A)[1]!=((INT*)&B)[1] ||
((INT*)&A)[2]!=((INT*)&B)[2] || ((INT*)&A)[3]!=((INT*)&B)[3];}
UBOOL NEQ(FString A,FString B,UPackageMap* Map) {return A!=B;}

#define DOREP(c,v) \
	if( NEQ(v,((A##c*)Recent)->v,Map) ) \
	{ \
		static UProperty* sp##v = FindObjectChecked<UProperty>(A##c::StaticClass(),TEXT(#v)); \
		*Ptr++ = sp##v->RepIndex; \
	}
#define DOREPARRAY(c,v) \
	static UProperty* sp##v = FindObjectChecked<UProperty>(A##c::StaticClass(),TEXT(#v)); \
	for( INT i=0; i<ARRAY_COUNT(v); i++ ) \
		if( NEQ(v[i],((A##c*)Recent)->v[i],Map) ) \
			*Ptr++ = sp##v->RepIndex+i;

INT* AActor::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AActor::GetOptimizedRepList);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			DOREP(Actor,Owner);
			DOREP(Actor,Role);
			DOREP(Actor,RemoteRole);
			DOREP(Actor,bCollideActors);
			DOREP(Actor,bCollideWorld);
			DOREP(Actor,LightType);
			DOREP(Actor,bHidden);
			DOREP(Actor,bOnlyOwnerSee);
			DOREP(Actor,Texture);
			DOREP(Actor,DrawScale);
			DOREP(Actor,DrawType);
			DOREP(Actor,Style);
			if( bNetOwner )
			{
				DOREP(Actor,bNetOwner);
				DOREP(Actor,Inventory);
			}
			if( bReplicateInstigator && RemoteRole>=ROLE_SimulatedProxy )
			{
				DOREP(Actor,Instigator);
			}
			if  ( !bNetOwner || !bClientAnim )
			{
				DOREP(Actor,AmbientSound);
				if( AmbientSound!=NULL )
				{
					DOREP(Actor,SoundRadius);
					DOREP(Actor,SoundVolume);
					DOREP(Actor,SoundPitch);
					DOREP(Actor,SoundOcclusion);
				}
			}

			if( bCollideActors || bCollideWorld )
			{
				DOREP(Actor,bProjTarget);
				DOREP(Actor,bBlockActors);
				DOREP(Actor,bBlockPlayers);
				DOREP(Actor,CollisionRadius);
				DOREP(Actor,CollisionHeight);
			}
			if( !bCarriedItem && (bNetInitial || bSimulatedPawn || RemoteRole<ROLE_SimulatedProxy) )
			{
				DOREP(Actor,Location);
				if( DrawType==DT_Mesh || DrawType==DT_Brush || DrawType==DT_Particles )
					DOREP(Actor,Rotation);
			}  
			if( DrawType==DT_Mesh )
			{
				DOREP(Actor,AmbientGlow);
				DOREP(Actor,ScaleGlow);
				DOREP(Actor,bUnlit);
				DOREP(Actor,Fatness);
				DOREP(Actor,Wideness);
				DOREP(Actor,PrePivot);
				DOREP(Actor,Mesh);
				DOREP(Actor,bMeshEnviroMap);
				DOREP(Actor,Skin);
				DOREPARRAY(Actor,MultiSkins);
				if( ((RemoteRole<=ROLE_SimulatedProxy) && (!bNetOwner || !bClientAnim)) || bDemoRecording )
				{
					DOREP(Actor,AnimSequence);
					DOREP(Actor,SimAnim);
					DOREP(Actor,AnimMinRate);
					DOREP(Actor,bAnimNotify);
				}
			}
			else if( DrawType==DT_Sprite )
			{
				if( !bHidden && (!bOnlyOwnerSee || bNetOwner) )
				{
					DOREP(Actor,Sprite);
				}
			}
			else if( DrawType==DT_Brush )
			{
				DOREP(Actor,PrePivot);
				DOREP(Actor,Brush);
			}
			if( LightType!=LT_None )
			{
				DOREP(Actor,LightEffect);
				DOREP(Actor,LightBrightness);
				DOREP(Actor,LightHue);
				DOREP(Actor,LightSaturation);
				DOREP(Actor,bDarkLight);
				DOREP(Actor,LightRadius);
				DOREP(Actor,LightRadiusInner);
				DOREP(Actor,LightPeriod);
				DOREP(Actor,LightPhase);
				DOREP(Actor,VolumeBrightness);
				DOREP(Actor,VolumeRadius);
				DOREP(Actor,bSpecialLit);
			}
			if( RemoteRole==ROLE_SimulatedProxy )
			{
				DOREP(Actor,Base);
				if( bNetInitial )
				{
					if( !bSimulatedPawn )
					{
						DOREP(Actor,Physics);
						DOREP(Actor,Acceleration);
						DOREP(Actor,bBounce);
					}
					if( Physics==PHYS_Rotating )
					{
						DOREP(Actor,bFixedRotationDir);
						DOREP(Actor,bRotateToDesired);
						DOREP(Actor,RotationRate);
						DOREP(Actor,DesiredRotation);
					}
				}
			}
			else if ( bSimFall )
			{
				DOREP(Actor,Physics);
				DOREP(Actor,Acceleration);
				DOREP(Actor,bBounce);
			}

			if( bSimFall || bIsMover || (RemoteRole==ROLE_SimulatedProxy && (bNetInitial || bSimulatedPawn)) )
			{
				DOREP(Actor,Velocity);
			}
		}
	}
	return Ptr;
	unguard;
}

FLOAT AActor::UpdateFrequency(AActor *Viewer, FVector &ViewDir, FVector &ViewPos)

{
	guard(AActor::UpdateFrequency);

	if ( bNetTemporary && !bNetOptional 
		&& (LifeSpan < GetClass()->GetDefaultActor()->LifeSpan - 0.2f)
		&& (((Location - ViewPos).SizeSquared() > 1000000.f)
			|| ((ViewDir | (Location - ViewPos)) < 0)) )
		return ::Min(NetUpdateFrequency, 8.f); 

	return NetUpdateFrequency;
	unguard;
}

INT* APawn::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APawn::GetOptimizedRepList);

	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if ( bNetOwner || bNetInitial || (NumReps%5 == 0) )

		{
			Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
			if( Role==ROLE_Authority )



			{
				DOREP(Pawn,PlayerReplicationInfo);
				DOREP(Pawn,Health);
				if( bNetOwner )
				{
					 DOREP(Pawn,bIsPlayer);
					 DOREP(Pawn,carriedDecoration);
					 DOREP(Pawn,MoveTarget);
					 DOREP(Pawn,SelectedItem);
					 DOREP(Pawn,GroundSpeed);
					 DOREP(Pawn,WaterSpeed);
					 DOREP(Pawn,AirSpeed);
					 DOREP(Pawn,AccelRate);
					 DOREP(Pawn,JumpZ);
					 DOREP(Pawn,AirControl);
					 DOREP(Pawn,bBehindView);
				}
			}
		}
		else
		{
			if( Role==ROLE_Authority )
			{
				DOREP(Actor,bHidden);
				if( RemoteRole<=ROLE_SimulatedProxy )
				{
					DOREP(Actor,Location);
					if( DrawType==DT_Mesh || DrawType==DT_Brush )
						DOREP(Actor,Rotation);
				}
				if( DrawType==DT_Mesh )
				{
					if( RemoteRole<=ROLE_SimulatedProxy || bDemoRecording )
					{
						DOREP(Actor,AnimSequence);
						DOREP(Actor,SimAnim);
						DOREP(Actor,AnimMinRate);
						DOREP(Actor,bAnimNotify);
					}
				}
				else if( DrawType==DT_Sprite )
				{
					if( !bHidden && !bOnlyOwnerSee )
						DOREP(Actor,Sprite);
				}
				if( RemoteRole==ROLE_SimulatedProxy )
				{
					DOREP(Actor,Base);
					DOREP(Actor,Velocity);
				}
				else if ( bSimFall )
				{
					DOREP(Actor,Physics);
					DOREP(Actor,Acceleration);
					DOREP(Actor,bBounce);
					DOREP(Actor,Velocity);
				}
			}
		}
		if ( Role == ROLE_Authority )
		{
			DOREP(Pawn,Weapon);
			DOREP(Pawn,bCanFly);
		}
		if( (bNetInitial && bNetOwner && bIsPlayer) || bDemoRecording )
		{
			DOREP(Pawn,ViewRotation);
		}
		if( bDemoRecording )
		{
			DOREP(Pawn,EyeHeight);
		}
	}
	else
		Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	return Ptr;
	unguard;
}

INT* APlayerPawn::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APlayerPawn::GetOptimizedRepList);
	Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			if( bNetOwner )
			{
				DOREP(PlayerPawn,ViewTarget);
				DOREP(PlayerPawn,ScoringType);
				DOREP(PlayerPawn,HUDType);
				DOREP(PlayerPawn,GameReplicationInfo);
				DOREP(PlayerPawn,bFixedCamera);
				DOREP(PlayerPawn,bNeverAutoSwitch);
				DOREP(PlayerPawn,bCheatsEnabled);
				DOREP(PlayerPawn,TargetViewRotation);
				DOREP(PlayerPawn,TargetEyeHeight);
				DOREP(PlayerPawn,TargetWeaponViewOffset);
			}
			if( bDemoRecording )
			{
				DOREP(PlayerPawn,DemoViewPitch);
				DOREP(PlayerPawn,DemoViewYaw);
			}
		}
		else
		{
			DOREP(PlayerPawn,Password);
			DOREP(PlayerPawn,bReadyToPlay);
		}
	}
	return Ptr;
	unguard;
}
INT* AMover::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AMover::GetOptimizedRepList);
	Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			DOREP(Mover,SimOldPos);
			DOREP(Mover,SimOldRotPitch);
			DOREP(Mover,SimOldRotYaw);
			DOREP(Mover,SimOldRotRoll);
			DOREP(Mover,SimInterpolate);
			DOREP(Mover,RealPosition);
			DOREP(Mover,RealRotation);
		}
	}
	return Ptr;
	unguard;
}

INT* AZoneInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AZoneInfo::GetOptimizedRepList);
	// only replicate needed actor properties
	if ( bNetInitial )
		Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			DOREP(ZoneInfo,ZoneGravity);
			DOREP(ZoneInfo,ZoneVelocity);
			DOREP(ZoneInfo,AmbientBrightness);
			DOREP(ZoneInfo,AmbientHue);
			DOREP(ZoneInfo,AmbientSaturation);
			DOREP(ZoneInfo,TexUPanSpeed);
			DOREP(ZoneInfo,TexVPanSpeed);
			DOREP(ZoneInfo,bReverbZone);
			DOREP(ZoneInfo,FogColor);

			DOREP(Actor,Role);
			DOREP(Actor,RemoteRole);

			if  ( !bNetOwner || !bClientAnim )
			{
				DOREP(Actor,AmbientSound);
				if( AmbientSound!=NULL )
				{
					DOREP(Actor,SoundRadius);
					DOREP(Actor,SoundVolume);
					DOREP(Actor,SoundPitch);
				}
			}	
		}
	}
	return Ptr;
	unguard;
}

INT* ALevelInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(ALevelInfo::GetOptimizedRepList);
	// only replicate needed actor properties
	Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			DOREP(LevelInfo,Pauser);
			DOREP(LevelInfo,TimeDilation);
			DOREP(LevelInfo,bNoCheating);
			DOREP(LevelInfo,bAllowFOV);
		}
	}
	return Ptr;
	unguard;
}

INT* APlayerReplicationInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APlayerReplicationInfo::GetOptimizedRepList);
	if ( bNetInitial )
		Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			DOREP(PlayerReplicationInfo,PlayerName);
			DOREP(PlayerReplicationInfo,OldName);
			DOREP(PlayerReplicationInfo,PlayerID);
			DOREP(PlayerReplicationInfo,TeamName);
			DOREP(PlayerReplicationInfo,Team);
			DOREP(PlayerReplicationInfo,TeamID);
			DOREP(PlayerReplicationInfo,Score);
			DOREP(PlayerReplicationInfo,Deaths);
			DOREP(PlayerReplicationInfo,VoiceType);
			DOREP(PlayerReplicationInfo,HasFlag);
			DOREP(PlayerReplicationInfo,Ping);
			DOREP(PlayerReplicationInfo,PacketLoss);
			DOREP(PlayerReplicationInfo,bIsFemale);
			DOREP(PlayerReplicationInfo,bIsABot);
			DOREP(PlayerReplicationInfo,bFeigningDeath);
			DOREP(PlayerReplicationInfo,bIsSpectator);
			DOREP(PlayerReplicationInfo,bWaitingPlayer);
			DOREP(PlayerReplicationInfo,bAdmin);
			DOREP(PlayerReplicationInfo,TalkTexture);
			DOREP(PlayerReplicationInfo,PlayerZone);
			DOREP(PlayerReplicationInfo,PlayerLocation);
			DOREP(PlayerReplicationInfo,StartTime);
		}
	}
	return Ptr;
	unguard;
}
INT* AGameReplicationInfo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AGameReplicationInfo::GetOptimizedRepList);
	if ( bNetInitial )
		Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( Role==ROLE_Authority )
		{
			DOREP(GameReplicationInfo,RemainingMinute);
			DOREP(GameReplicationInfo,bStopCountDown);
			DOREP(GameReplicationInfo,GameEndedComments);
			DOREP(GameReplicationInfo,NumPlayers);
			if ( bNetInitial )
			{
				DOREP(GameReplicationInfo,GameName);
				DOREP(GameReplicationInfo,GameClass);
				DOREP(GameReplicationInfo,bTeamGame);
				DOREP(GameReplicationInfo,RemainingTime);
				DOREP(GameReplicationInfo,ElapsedTime);
				DOREP(GameReplicationInfo,ServerName);
				DOREP(GameReplicationInfo,ShortName);
				DOREP(GameReplicationInfo,AdminName);
				DOREP(GameReplicationInfo,AdminEmail);
				DOREP(GameReplicationInfo,MOTDLine1);
				DOREP(GameReplicationInfo,MOTDLine2);
				DOREP(GameReplicationInfo,MOTDLine3);
				DOREP(GameReplicationInfo,MOTDLine4);
			}
		}
	}
	return Ptr;
	unguard;
}
INT* AInventory::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AInventory::GetOptimizedRepList);

	if ( bAlwaysRelevant && !bNetInitial ) // only inventory pickups should be like this
	{
			DOREP(Actor,bHidden);
			return Ptr;
	}
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if ( bNetInitial || (NumReps % 5 == 0) ) 
		{
			Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
			if( bNetOwner )
			{
				DOREP(Inventory,bIsAnArmor);
				DOREP(Inventory,Charge);
				DOREP(Inventory,bActivatable);
				DOREP(Inventory,bActive);
				DOREP(Inventory,PlayerViewOffset);
				DOREP(Inventory,PlayerViewMesh);
				DOREP(Inventory,PlayerViewScale);
			}
		}
		else
		{
			if( Role==ROLE_Authority )
			{
				DOREP(Actor,LightType);
				DOREP(Actor,bCollideWorld);
				DOREP(Actor,bHidden);
				if( bNetOwner )
					DOREP(Actor,Inventory);
				if( !bCarriedItem && (RemoteRole<ROLE_SimulatedProxy) )
				{
					DOREP(Actor,Location);
					if( DrawType==DT_Mesh || DrawType==DT_Brush )
						DOREP(Actor,Rotation);
				}
				if( DrawType==DT_Mesh )
				{
					DOREP(Actor,bUnlit);
					if( ((RemoteRole<=ROLE_SimulatedProxy) && (!bNetOwner || !bClientAnim)) || bDemoRecording )
					{
						DOREP(Actor,AnimSequence);
						DOREP(Actor,SimAnim);
						DOREP(Actor,AnimMinRate);
						DOREP(Actor,bAnimNotify);
					}
				}
				else if( DrawType==DT_Sprite )
				{
					if( !bHidden && (!bOnlyOwnerSee || bNetOwner) )
						DOREP(Actor,Sprite);
				}
				if( LightType!=LT_None )
				{
					DOREP(Actor,LightEffect);
					DOREP(Actor,LightBrightness);
					DOREP(Actor,LightHue);
					DOREP(Actor,LightSaturation);
					DOREP(Actor,LightRadius);
				}
				if( RemoteRole==ROLE_SimulatedProxy )
				{
					DOREP(Actor,Base);
				}
				else if ( bSimFall )
				{
					DOREP(Actor,Physics);
					DOREP(Actor,Acceleration);
					DOREP(Actor,bBounce);
					DOREP(Actor,Velocity);
				}
				if( bNetOwner && Instigator && (Instigator->Weapon == this) )
				{
					DOREP(Inventory,PlayerViewOffset);
					DOREP(Inventory,PlayerViewMesh);
					DOREP(Inventory,PlayerViewScale);
				}
			}
		}
		if( Role==ROLE_Authority )
		{
			if ( !bNetOwner && (RemoteRole == ROLE_SimulatedProxy) && AmbientSound )
				DOREP(Actor,Location);
			DOREP(Inventory,FlashCount);
			DOREP(Inventory,bSteadyFlash3rd);
			DOREP(Inventory,ThirdPersonMesh);
			DOREP(Inventory,ThirdPersonScale);
		}
	}
	else
		Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	return Ptr;
	unguard;
}
UBOOL AInventory::ShouldDoScriptReplication()
{
	return bNetOwner || bNetInitial; //!bAlwaysRelevant || bNetInitial;
}


UBOOL AInventory::NoVariablesToReplicate(AActor *OldVer)
{
	return (bHidden == OldVer->bHidden);
}

FLOAT AInventory::UpdateFrequency(AActor *Viewer, FVector &ViewDir, FVector &ViewPos)
{
	guard(AInventory::UpdateFrequency);

	if ( bHidden && Owner && Owner->IsA(APawn::StaticClass()) )
	{
		if ( this != ((APawn *)Owner)->Weapon )
			return ::Min(NetUpdateFrequency, 4.f);
		
		// if its someone elses weapon, update less frequently
		if ( Owner != Viewer )
			return ::Min(NetUpdateFrequency, 10.f);
	}

	return NetUpdateFrequency;
	unguard;
}

FLOAT ACarcass::UpdateFrequency(AActor *Viewer, FVector &ViewDir, FVector &ViewPos)
{
	guard(ACarcass::UpdateFrequency);

	if ( (Physics == PHYS_None) && !IsAnimating() )
		return ::Min(NetUpdateFrequency, 4.f);
	return NetUpdateFrequency;
	unguard;
}

INT* AWeapon::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AWeapon::GetOptimizedRepList);

	if ( bAlwaysRelevant && !bNetInitial ) // only inventory pickups should be like this
	{
			DOREP(Actor,bHidden);
			return Ptr;
	}
	Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( bNetOwner && (Role==ROLE_Authority) )
		{
			DOREP(Weapon,AmmoType);
			DOREP(Weapon,bLockedOn);
			DOREP(Weapon,bHideWeapon);
		}
	}
	return Ptr;
	unguard;
}

INT* AAmmo::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(AAmmo::GetOptimizedRepList);

	if ( bAlwaysRelevant && !bNetInitial ) // only inventory pickups should be like this
	{
			DOREP(Actor,bHidden);
			return Ptr;
	}
	Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( bNetOwner && (Role==ROLE_Authority) )
			DOREP(Ammo,AmmoAmount);
	}
	return Ptr;
	unguard;
}

INT* APickup::GetOptimizedRepList( BYTE* Recent, FPropertyRetirement* Retire, INT* Ptr, UPackageMap* Map, INT NumReps )
{
	guard(APickup::GetOptimizedRepList);

	if ( bAlwaysRelevant && !bNetInitial ) // only inventory pickups should be like this
	{
			DOREP(Actor,bHidden);
			return Ptr;
	}
	Ptr = Super::GetOptimizedRepList(Recent,Retire,Ptr,Map,NumReps);
	if( StaticClass()->ClassFlags & CLASS_NativeReplication )
	{
		if( bNetOwner && (Role==ROLE_Authority) )
			DOREP(Pickup, NumCopies);
	}
	return Ptr;
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
	if( Index < ARRAY_COUNT(MultiSkins) )
		return MultiSkins[Index];
	return NULL;
}

//
// Net priority.
//
FLOAT AActor::GetNetPriority( AActor* Sent, FLOAT Time, FLOAT Lag )
{
	guardSlow(AActor::GetNetPriority);
	if ( bAlwaysRelevant )
		return NetPriority * Time * ::Min(NetUpdateFrequency * 0.1f, 1.f);
	else
		return NetPriority * Time;
	unguardSlow;
}

//
// Always called immediately before properties are received from the remote.
//
void AActor::PreNetReceive()
{
	guard(AActor::PreNetReceive);
	SavedLocation   = Location;
	SavedRotation   = Rotation;
	SavedBase       = Base;
	SavedCollision  = bCollideActors;
	SavedRadius		= CollisionRadius;
	SavedHeight     = CollisionHeight;
	SavedSimAnim    = SimAnim;
	if( IsA(AMover::StaticClass()) )
		SavedSimInterpolate = ((AMover*)this)->SimInterpolate;
	if( bCollideActors )
		GetLevel()->Hash->RemoveActor( this );
	unguard;
}

//
// Always called immediately after properties are received from the remote.
//
void AActor::PostNetReceive()
{
	guard(AActor::PostNetReceive);
	Exchange ( Location,        SavedLocation  );
	Exchange ( Rotation,        SavedRotation  );
	Exchange ( Base,            SavedBase      );
	ExchangeB( bCollideActors,  SavedCollision );
	Exchange ( CollisionRadius, SavedRadius    );
	Exchange ( CollisionHeight, SavedHeight    );
	if( bCollideActors )
		GetLevel()->Hash->AddActor( this );
	if( IsA(AMover::StaticClass()) )
	{
		AMover* Mover = Cast<AMover>( this );
		if( SavedSimInterpolate != Mover->SimInterpolate )
		{
			Mover->OldPos = Mover->SimOldPos;
			Mover->OldRot.Yaw = Mover->SimOldRotYaw;
			Mover->OldRot.Pitch = Mover->SimOldRotPitch;
			Mover->OldRot.Roll = Mover->SimOldRotRoll;
			Mover->PhysAlpha = Mover->SimInterpolate.X * 0.01f;
			Mover->PhysRate = Mover->SimInterpolate.Y * 0.01f;
			INT keynums = (INT) Mover->SimInterpolate.Z;
			Mover->KeyNum = keynums & 255;
			Mover->PrevKeyNum = keynums >> 8;
			Mover->setPhysics(PHYS_MovingBrush);
			Mover->bInterpolating   = true;
			/*FRotator ApproxRot = Mover->BaseRot + Mover->KeyRot[Mover->KeyNum];
			ApproxRot.Roll = ApproxRot.Roll & 65280;
			ApproxRot.Pitch = ApproxRot.Pitch & 65280;
			ApproxRot.Yaw = ApproxRot.Yaw & 65280;
			if ( ApproxRot == Mover->OldRot )
				Mover->OldRot = Mover->BaseRot + Mover->KeyRot[Mover->KeyNum];*/
		}
	}
	if( IsA(APlayerPawn::StaticClass()) && GetLevel()->DemoRecDriver && GetLevel()->DemoRecDriver->ServerConnection )
	{
		APlayerPawn* PlayerPawn = Cast<APlayerPawn>( this );
		PlayerPawn->ViewRotation.Pitch = PlayerPawn->DemoViewPitch;
		PlayerPawn->ViewRotation.Yaw = PlayerPawn->DemoViewYaw;
	}
	if( SimAnim != SavedSimAnim )
	{
		AnimFrame = SimAnim.X * 0.0001f;
		AnimRate  = SimAnim.Y * 0.0002f;
		TweenRate = SimAnim.Z * 0.001f;
		AnimLast  = SimAnim.W * 0.0001f;
		if( AnimLast < 0 )
		{
			AnimLast *= -1;
			bAnimLoop = 1;
			if( IsA(APawn::StaticClass()) && AnimMinRate<0.5f )
				AnimMinRate = 0.5f;
		}
		else bAnimLoop = 0;
	}
	if( Location!=SavedLocation )
	{
		if( IsA(APawn::StaticClass()) && (Role == ROLE_SimulatedProxy) 
			&& !Velocity.IsNearlyZero() && ((Location - SavedLocation).SizeSquared() < 10000) )
		{
			// smooth out movement of other players to account for frame rate induced jitter
			// look at whether location is a reasonable approximation already (<100 error)
			// if so only partially correct
			
			FLOAT StartError = (Location - SavedLocation).SizeSquared();
			FCheckResult Hit(1.f);
			FVector NewLocation = Location;
			if ( StartError > 1600 )
			{
				// if error > 40 try moving smoothly closer
				moveSmooth(0.35f * (SavedLocation - Location));
				// if error not reduced enough, set to new loc
				if ( (Location - SavedLocation).SizeSquared() > 0.75f * StartError )
					NewLocation += + 0.5f * (SavedLocation - Location);
			}
			else
				NewLocation += 0.15f * (SavedLocation - Location);

			GetLevel()->FarMoveActor( this, NewLocation, 0, 1 );
		}
		else
			GetLevel()->FarMoveActor( this, SavedLocation, 0, 1 );
	}
	if( Rotation!=SavedRotation )
	{
		FCheckResult Hit;
		GetLevel()->MoveActor( this, FVector(0,0,0), SavedRotation, Hit, 0, 0, 0, 1 );
	}
	if( CollisionRadius!=SavedRadius || CollisionHeight!=SavedHeight )
	{
		SetCollisionSize( SavedRadius, SavedHeight );
	}
	if( bCollideActors!=SavedCollision )
	{
		SetCollision( SavedCollision, bBlockActors, bBlockPlayers );
	}
	if( Base!=SavedBase )
	{
		// Base changed.
		eventBump( SavedBase );
		if( SavedBase )
			SavedBase->eventBump( this );
		SetBase( SavedBase );
	}
	bJustTeleported = 0;
	if (IsA(APlayerReplicationInfo::StaticClass()) && Level->NetMode == NM_Client)
	{
		APlayerReplicationInfo* PRI = Cast<APlayerReplicationInfo>( this );
		if( GetLevel()->NetDriver &&
			GetLevel()->Engine->Client->Viewports(0)->Actor &&
			GetLevel()->Engine->Client->Viewports(0)->Actor->PlayerReplicationInfo == this
		)
			PRI->Ping -= (INT) ((GetLevel()->NetDriver->ServerConnection->AverageFrameTime * 1000) / 2);
		if( PRI->Ping < 0 )
			PRI->Ping = 0;
	}
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
	check(InPlayer!=NULL);

	// Detach old player.
	if( InPlayer->Actor )
	{
		InPlayer->Actor->Player = NULL;
		InPlayer->Actor = NULL;
	}

	// Set the viewport.
	Player = InPlayer;
	InPlayer->Actor = this;

	// Send possess message to script.
	eventPossess();

	// Debug message.
	debugf( NAME_Log, TEXT("Possessed PlayerPawn: %s"), GetFullName() );

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
	if( GIsEditor )
		GCache.Flush();
	unguard;
}

/*-----------------------------------------------------------------------------
	AActor.
-----------------------------------------------------------------------------*/

void AActor::Destroy()
{
	guard(AActor::Destroy);
	if( RenderInterface )
	{
		RenderInterface->RemoveFromRoot();
		RenderInterface->ConditionalDestroy();
		delete RenderInterface;
		RenderInterface = NULL;
	}
	for_array( a, AuxAnims )
		if( AuxAnims(a) )
			AuxAnims(a)->ConditionalDestroy();
	UObject::Destroy();
	unguard;
}

void AActor::PostLoad()
{
	guard(AActor::PostLoad);
	Super::PostLoad();
	if( GetClass()->ClassFlags & CLASS_Localized )
		LoadLocalized();
	if( Brush )
		Brush->SetFlags( RF_Transactional );
	if( Brush && Brush->Polys )
		Brush->Polys->SetFlags( RF_Transactional );
	if( CollideType == CT_Shape && (Brush || Mesh) )
		GetPrimitive()->ValidateActor(this);
	unguard;
}

void AActor::Spawned()
{
	if( CollideType == CT_Shape )
		GetPrimitive()->ValidateActor(this);
}

void AActor::ProcessEvent( UFunction* Function, void* Parms, void* Result )
{
	guardSlow(AActor::ProcessEvent);
	if( Level->bBegunPlay )
		Super::ProcessEvent( Function, Parms, Result );
	unguardSlow;
}

void AActor::PostEditChange()
{
	guard(AActor::PostEditChange);
	Super::PostEditChange();
	if( GIsEditor )
		bLightChanged = 1;
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

	// Untouch this actor.
	if( bCollideActors && GetLevel()->Hash )
		GetLevel()->Hash->RemoveActor( this );

	// Set properties.
	bCollideActors = NewCollideActors;
	bBlockActors   = NewBlockActors;
	bBlockPlayers  = NewBlockPlayers;

	// Touch this actor.
	if( bCollideActors && GetLevel()->Hash )
		GetLevel()->Hash->AddActor( this );

	unguard;
}

//
// Set collision size.
//
void AActor::SetCollisionSize( FLOAT NewRadius, FLOAT NewHeight )
{
	guard(AActor::SetCollisionSize);

	// Untouch this actor.
	if( bCollideActors && GetLevel()->Hash )
		GetLevel()->Hash->RemoveActor( this );

	// Set properties.
	CollisionRadius = NewRadius;
	CollisionHeight = NewHeight;

	// Touch this actor.
	if( bCollideActors && GetLevel()->Hash )
		GetLevel()->Hash->AddActor( this );

	unguard;
}

//
// Return whether this actor overlaps another.
//
UBOOL AActor::IsOverlapping( const AActor* Other ) const
{
	guardSlow(AActor::IsOverlapping);
	checkSlow(Other!=NULL);

	if( !IsBrush() && !Other->IsBrush() && Other!=Level )
	{
		// See if cylinder actors are overlapping.
		return
			Square(Location.X      - Other->Location.X)
		+	Square(Location.Y      - Other->Location.Y)
		<	Square(CollisionRadius + Other->CollisionRadius) 
		&&	Square(Location.Z      - Other->Location.Z)
		<	Square(CollisionHeight + Other->CollisionHeight);
	}
	else
	{
		// We cannot detect whether these actors are overlapping so we say they aren't.
		return 0;
	}
	unguardSlow;
}

//
// Return visiblity box.
//
FBox AActor::GetVisibilityBox()
{
	guard(AActor::GetVisibilityBox);
	FVector Extent( VisibilityRadius+1, VisibilityRadius+1, VisibilityHeight+1 );
	return FBox( Location - Extent, Location + Extent );
	unguard;
}

/*-----------------------------------------------------------------------------
	Actor touch minions.
-----------------------------------------------------------------------------*/

static UBOOL TouchTo( AActor* Actor, AActor* Other )
{
	guard(TouchTo);
	check(Actor);
	check(Other);
	check(Actor!=Other);

	INT Available=-1;
	for( INT i=0; i<ARRAY_COUNT(Actor->Touching); i++ )
	{
		if( Actor->Touching[i] == NULL )
		{
			// Found an available slot.
			Available = i;
		}
		else if( Actor->Touching[i] == Other )
		{
			// Already touching.
			return 1;
		}
	}
	if( Available == -1 )
	{
		// Try to prune touches.
		for( INT i=0; i<ARRAY_COUNT(Actor->Touching); i++ )
		{
			check(Actor->Touching[i]->IsValid());
			if( Actor->Touching[i]->Physics == PHYS_None )
			{
				Actor->EndTouch( Actor->Touching[i], 0 );
				Available = i;
			}
		}
		if ( (Available == -1) && Other->IsA(APawn::StaticClass()) )
		{
			// try to prune in favor of 1. players, 2. other pawns
			for( INT i=0; i<ARRAY_COUNT(Actor->Touching); i++ )
			{
				if( !Actor->Touching[i]->IsA(APawn::StaticClass()) )
				{
					Actor->EndTouch( Actor->Touching[i], 0 );
					Available = i;
					break;
				}
			}
			if ( (Available == -1) && ((APawn *)Other)->bIsPlayer )
				for( INT i=0; i<ARRAY_COUNT(Actor->Touching); i++ )
				{
					if( !Actor->Touching[i]->IsA(APawn::StaticClass()) || !((APawn *)Actor->Touching[i])->bIsPlayer )
					{
						Actor->EndTouch( Actor->Touching[i], 0 );
						Available = i;
						break;
					}
				}
		}
	}

	if( Available >= 0 )
	{
		// Make Actor touch TouchActor.
		Actor->Touching[Available] = Other;
		Actor->eventTouch( Other );

		// See if first actor did something that caused an UnTouch.
		if( Actor->Touching[Available] != Other )
			return 0;
	}

	return 1;
	unguard;
}

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

	// Spell projectiles destroy themselves from ProcessTouch. Notify shoot
	// triggers first so their reflective Touch callback cannot be suppressed
	// when projectile teardown clears the temporary Touching entry.
	if( IsA(AProjectile::StaticClass()) && Other->IsA(ATrigger::StaticClass()) )
	{
		TouchTo( Other, this );
		if( !bDeleteMe && !Other->bDeleteMe )
			TouchTo( this, Other );
	}
	else if( Other->IsA(AProjectile::StaticClass()) && IsA(ATrigger::StaticClass()) )
	{
		TouchTo( this, Other );
		if( !bDeleteMe && !Other->bDeleteMe )
			TouchTo( Other, this );
	}
	else if( TouchTo( this, Other ) )
	{
		TouchTo( Other, this );
	}

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

	// Notify Actor.
	for( int i=0; i<ARRAY_COUNT(Touching); i++ )
	{
		if( Touching[i] == Other )
		{
			Touching[i] = NULL;
			if( !NoNotifySelf )
				eventUnTouch( Other );
			break;
		}
	}

	// Notify TouchActor.
	for( int i=0; i<ARRAY_COUNT(Other->Touching); i++ )
	{
		if( Other->Touching[i] == this )
		{
			Other->Touching[i] = NULL;
			Other->eventUnTouch( this );
			break;
		}
	}
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

	SetFlags( RF_DebugSerialize );

	UClass*		 TheClass	   = GetClass();

	// Make sure this object's class's data is loaded.
	if( TheClass != UClass::StaticClass() )
		Ar.Preload( TheClass );

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
	// *** Serialize PROPERTIES *** (defined in script )

	// Serialize object properties which are defined in the class.
	if( TheClass != UClass::StaticClass() )
	{
		// serialize properties
		if( (Ar.IsLoading() || Ar.IsSaving()) && !Ar.IsTrans() )
			GetClass()->SerializeTaggedPersistentProperties( Ar, (BYTE*)this, GetClass() );
		else
			GetClass()->SerializeBin( Ar, (BYTE*)this );
	}
	

	// Memory counting.
	SIZE_T Size = GetClass()->GetPropertiesSize();
	Ar.CountBytes( Size, Size );

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
	if( Owner != NULL )
		Owner->eventLostChild( this );

	Owner = NewOwner;

	if( Owner != NULL )
		Owner->eventGainedChild( this );

	if( NewOwner == NULL )
		AnimBone = 0;

	unguard;
}

//
// Change the actor's base.
//
void AActor::SetBase( AActor* NewBase, int bNotifyActor )
{
	guard(AActor::SetBase);
	//debugf("SetBase %s -> %s",GetName(),NewBase ? NewBase->GetName() : TEXT("NULL"));

	// Verify no recursion.
	for( AActor* Loop=NewBase; Loop!=NULL; Loop=Loop->Base )
		if ( Loop == this ) 
			return;

	if( NewBase != Base )
	{
		// Notify old base, unless it's the level.
		if( Base && Base!=Level )
		{
			Base->StandingCount--;
			Base->eventDetach( this );
		}

		// Set base.
		Base = NewBase;

		// Notify new base, unless it's the level.
		if( Base && Base!=Level )
		{
			Base->StandingCount++;
			Base->eventAttach( this );
		}

		// Notify this actor of his new floor.
		if ( bNotifyActor )
			eventBaseChange();
	}
	unguard;
}

//
// Get the actor's primitive.
//
UPrimitive* AActor::GetPrimitive() const
{
	guardSlow(AActor::GetPrimitive);
	if( CollideType == CT_Shape )
	{
		// Use shape primitive.
		if( Mesh ) 
			return Mesh;
		if ( Brush  ) 
			return Brush;
	}

	switch( CollideType )
	{
		default:
		case CT_AlignedCylinder:
			checkSlow( GetLevel()->Engine->Cylinder );
			return GetLevel()->Engine->Cylinder;
		case CT_OrientedCylinder:
			checkSlow( GetLevel()->Engine->OCylinder );
			return GetLevel()->Engine->OCylinder;
		case CT_AlignedOvalCylinder:
			checkSlow( GetLevel()->Engine->OvalCylinder );
			return GetLevel()->Engine->OvalCylinder;
		case CT_OrientedOvalCylinder:
			checkSlow( GetLevel()->Engine->OOvalCylinder );
			return GetLevel()->Engine->OOvalCylinder;
		case CT_Box:
			checkSlow( GetLevel()->Engine->Box );
			return GetLevel()->Engine->Box;
	}
	unguardSlow;
}

FVector AActor::GetCylinderExtent() const
{
	// This is handled here for all collision types, 
	// rather than using GetPrimitive()->GetCollisionExtent(),
	// so that it can be called on a default object using non-virtual function.
	guardSlow(AActor::GetCylinderExtent);
	if( CollideType == CT_Shape )
	{
		// Use shape primitive.
		if( Mesh ) 
			return Mesh->GetCollisionExtent(this);
		if ( Brush  ) 
			return Brush->GetCollisionExtent(this);
	}

	if( CollideType == CT_Box )
		return FVector( CollisionRadius, CollisionWidth ? CollisionWidth : CollisionRadius, CollisionHeight );
	else
		return FVector( CollisionRadius, CollisionRadius, CollisionHeight );
	unguardSlow;
}

FCoords AActor::GetRenderBoundingBox( UBOOL Exact )
{
	if( Mesh )
		return Mesh->GetRenderBoundingBox( this, Exact );
	else if( Brush )
		return Brush->GetRenderBoundingBox( this, Exact ).GetCoords();
	else
		return FCoords(0);
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

	// Trace the line.
	FCheckResult Hit(1.0);

	GetLevel()->SingleLineCheck(Hit, this, TraceEnd, TraceStart, TRACE_Level, FVector(0, 0, 0));

	UModel* Model = XLevel->Model;
	UTexture *Texture = NULL;

	//rb	*ScrollDir = FVector(0, 0, 0);
	*Flags = 0;

#if 0
	if ( Hit.Actor == NULL )
		debugf(TEXT("TraceTexture found nothing"));
#endif

	// Determine the node the trace actually hit (because the current Node could
	// just be the first node in a number of coplanar nodes)
	const FBspSurf* Surf = Hit.BspSurf();
	if (Surf)
	{
		Texture = Surf->Texture;
		*Flags = Surf->PolyFlags | (Surf->Texture ? Surf->Texture->PolyFlags : 0);

		if( bTraceDecals && Surf->Decals.Num() )
		{
			FVector &SurfNormal = Model->Vectors(Surf->vNormal);
			FVector &SurfBase = Model->Points(Surf->pBase);

			for( INT d=0; d < Surf->Decals.Num(); ++d )
			{
				const FDecal& Decal = Surf->Decals(d);
/*
				UViewport* Viewport = GetLevel()->Engine->Client->Viewports(0);
				if( Decal.Actor->bScryeOnly && Viewport->Actor->ScryeFraction( Decal.Actor ) == 0.0f )
					continue;
*/
				if( PointInsideQuad( Hit.Location - SurfBase, Decal.Vertices, SurfNormal ) && Decal.Actor->Texture )
				{
					Texture = Decal.Actor->Texture;
				}
			}
		}

		/* //rb 
		if(Surf->PolyFlags & PF_AutoUPan)
		{
			*ScrollDir += Model->Vectors(Surf->vTextureU) * Region.Zone->TexUPanSpeed; 
		}
		if(Surf->PolyFlags & PF_AutoVPan)
		{ 
			*ScrollDir += Model->Vectors(Surf->vTextureV) * Region.Zone->TexVPanSpeed;
		}
		*/
	}

	*(UTexture **)Result = Texture;

	unguardexecSlow;
}



/*-----------------------------------------------------------------------------
	Special editor support.
-----------------------------------------------------------------------------*/
AActor* AActor::GetHitActor()
{
	if (HitActor)
		return HitActor;
	return this;
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/
