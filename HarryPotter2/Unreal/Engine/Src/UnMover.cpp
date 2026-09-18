/*=============================================================================
	UnMover.cpp: Keyframe mover actor code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"

/*-----------------------------------------------------------------------------
	AMover implementation.
-----------------------------------------------------------------------------*/

AMover::AMover()
{}
void AMover::StaticConstructor()
{
	guard(AMover::StaticConstructor);

	UStruct* VectorStruct = FindObject<UStruct>(UObject::StaticClass(),TEXT("Vector"));
	if( !VectorStruct )
	{
		VectorStruct = new(UObject::StaticClass(),TEXT("Vector"),RF_Public)UStruct(NULL);
		VectorStruct->SetPropertiesSize(sizeof(FVector));
		new(VectorStruct,TEXT("X"),RF_Public)UFloatProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FVector,X)),TEXT(""),0);
		new(VectorStruct,TEXT("Y"),RF_Public)UFloatProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FVector,Y)),TEXT(""),0);
		new(VectorStruct,TEXT("Z"),RF_Public)UFloatProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FVector,Z)),TEXT(""),0);
		FArchive ArDummy;
		VectorStruct->Link(ArDummy,0);
	}
	UStruct* RotatorStruct = FindObject<UStruct>(UObject::StaticClass(),TEXT("Rotator"));
	if( !RotatorStruct )
	{
		RotatorStruct = new(UObject::StaticClass(),TEXT("Rotator"),RF_Public)UStruct(NULL);
		RotatorStruct->SetPropertiesSize(sizeof(FRotator));
		new(RotatorStruct,TEXT("Pitch"),RF_Public)UIntProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FRotator,Pitch)),TEXT(""),0);
		new(RotatorStruct,TEXT("Yaw"),RF_Public)UIntProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FRotator,Yaw)),TEXT(""),0);
		new(RotatorStruct,TEXT("Roll"),RF_Public)UIntProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FRotator,Roll)),TEXT(""),0);
		FArchive ArDummy;
		RotatorStruct->Link(ArDummy,0);
	}
	UEnum* EncroachTypes = new(GetClass(),TEXT("EMoverEncroachType"))UEnum(NULL);
	new(EncroachTypes->Names)FName(TEXT("ME_StopWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_ReturnWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_CrushWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_IgnoreWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_ReallyStopWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_ReallyReturnWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_ReallyCrushWhenEncroach"));
	new(EncroachTypes->Names)FName(TEXT("ME_ReallyIgnoreWhenEncroach"));
	UEnum* GlideTypes = new(GetClass(),TEXT("EMoverGlideType"))UEnum(NULL);
	new(GlideTypes->Names)FName(TEXT("MV_MoveByTime"));
	new(GlideTypes->Names)FName(TEXT("MV_GlideByTime"));
	new(GlideTypes->Names)FName(TEXT("MV_SpringByTime"));
	UEnum* BumpTypes = new(GetClass(),TEXT("EBumpType"))UEnum(NULL);
	new(BumpTypes->Names)FName(TEXT("BT_PlayerBump"));
	new(BumpTypes->Names)FName(TEXT("BT_PawnBump"));
	new(BumpTypes->Names)FName(TEXT("BT_AnyBump"));


#define REGISTER_MOVER_PROPERTY(PropertyType,Member) \
	new(GetClass(),TEXT(#Member),RF_Public)PropertyType(CPP_PROPERTY(Member),TEXT("Mover"),0)
#define REGISTER_MOVER_ARRAY_PROPERTY(PropertyType,Member,Count) \
	{ UProperty* Property = REGISTER_MOVER_PROPERTY(PropertyType,Member); Property->ArrayDim = Count; }
#define REGISTER_MOVER_OBJECT(Member,ObjectClass) \
	new(GetClass(),TEXT(#Member),RF_Public)UObjectProperty(CPP_PROPERTY(Member),TEXT("Mover"),0,ObjectClass)
#define REGISTER_MOVER_STRUCT(Member,StructMetadata) \
	new(GetClass(),TEXT(#Member),RF_Public)UStructProperty(CPP_PROPERTY(Member),TEXT("Mover"),0,StructMetadata)
#define REGISTER_MOVER_STRUCT_ARRAY(Member,StructMetadata,Count) \
	{ UProperty* Property = REGISTER_MOVER_STRUCT(Member,StructMetadata); Property->ArrayDim = Count; }
#define REGISTER_MOVER_BOOL(Member,StorageOffset,Mask) \
	{ UBoolProperty* Property = new(GetClass(),TEXT(#Member),RF_Public)UBoolProperty(EC_CppProperty,StorageOffset,TEXT("Mover"),0); Property->BitMask = Mask; }

	new(GetClass(),TEXT("MoverEncroachType"),RF_Public)UByteProperty(CPP_PROPERTY(MoverEncroachType),TEXT("Mover"),0,EncroachTypes);
	new(GetClass(),TEXT("MoverGlideType"),RF_Public)UByteProperty(CPP_PROPERTY(MoverGlideType),TEXT("Mover"),0,GlideTypes);
	new(GetClass(),TEXT("BumpType"),RF_Public)UByteProperty(CPP_PROPERTY(BumpType),TEXT("Mover"),0,BumpTypes);
	REGISTER_MOVER_PROPERTY(UByteProperty,KeyNum);
	REGISTER_MOVER_PROPERTY(UByteProperty,PrevKeyNum);
	REGISTER_MOVER_PROPERTY(UByteProperty,NumKeys);
	REGISTER_MOVER_PROPERTY(UByteProperty,WorldRaytraceKey);
	REGISTER_MOVER_PROPERTY(UByteProperty,BrushRaytraceKey);
	REGISTER_MOVER_ARRAY_PROPERTY(UFloatProperty,MoveTimes,8);
	REGISTER_MOVER_PROPERTY(UFloatProperty,MoveTime);
	REGISTER_MOVER_PROPERTY(UFloatProperty,StayOpenTime);
	REGISTER_MOVER_PROPERTY(UFloatProperty,OtherTime);
	REGISTER_MOVER_PROPERTY(UIntProperty,EncroachDamage);

	const INT StateBoolOffset = static_cast<INT>(__builtin_offsetof(AMover,PlayerBumpEvent) - sizeof(BITFIELD));
	REGISTER_MOVER_BOOL(bKeepRotationDirection,StateBoolOffset,1);
	REGISTER_MOVER_BOOL(bTriggerOnceOnly,StateBoolOffset,2);
	REGISTER_MOVER_BOOL(bSlave,StateBoolOffset,4);
	REGISTER_MOVER_BOOL(bUseTriggered,StateBoolOffset,8);
	REGISTER_MOVER_BOOL(bDamageTriggered,StateBoolOffset,16);
	REGISTER_MOVER_BOOL(bDynamicLightMover,StateBoolOffset,32);

	REGISTER_MOVER_PROPERTY(UNameProperty,PlayerBumpEvent);
	REGISTER_MOVER_PROPERTY(UNameProperty,BumpEvent);
	REGISTER_MOVER_OBJECT(SavedTrigger,AActor::StaticClass());
	REGISTER_MOVER_PROPERTY(UFloatProperty,DamageThreshold);
	REGISTER_MOVER_PROPERTY(UIntProperty,numTriggerEvents);
	REGISTER_MOVER_OBJECT(Leader,GetClass());
	REGISTER_MOVER_OBJECT(Follower,GetClass());
	REGISTER_MOVER_PROPERTY(UNameProperty,ReturnGroup);
	REGISTER_MOVER_PROPERTY(UFloatProperty,DelayTime);
	REGISTER_MOVER_PROPERTY(UNameProperty,AttachTag);
	REGISTER_MOVER_OBJECT(OpeningSound,USound::StaticClass());
	REGISTER_MOVER_OBJECT(OpenedSound,USound::StaticClass());
	REGISTER_MOVER_OBJECT(ClosingSound,USound::StaticClass());
	REGISTER_MOVER_OBJECT(ClosedSound,USound::StaticClass());
	REGISTER_MOVER_OBJECT(MoveAmbientSound,USound::StaticClass());
	REGISTER_MOVER_OBJECT(FailSound,USound::StaticClass());
	REGISTER_MOVER_PROPERTY(UFloatProperty,MoverRadius);
	REGISTER_MOVER_PROPERTY(UByteProperty,MoverVolume);
	REGISTER_MOVER_PROPERTY(UByteProperty,MoverPitch);

	REGISTER_MOVER_STRUCT_ARRAY(KeyPos,VectorStruct,8);
	REGISTER_MOVER_STRUCT_ARRAY(KeyRot,RotatorStruct,8);
	REGISTER_MOVER_STRUCT(BasePos,VectorStruct);
	REGISTER_MOVER_STRUCT(OldPos,VectorStruct);
	REGISTER_MOVER_STRUCT(OldPrePivot,VectorStruct);
	REGISTER_MOVER_STRUCT(SavedPos,VectorStruct);
	REGISTER_MOVER_STRUCT(BaseRot,RotatorStruct);
	REGISTER_MOVER_STRUCT(OldRot,RotatorStruct);
	REGISTER_MOVER_STRUCT(SavedRot,RotatorStruct);
	REGISTER_MOVER_PROPERTY(UFloatProperty,PhysAlpha);
	REGISTER_MOVER_PROPERTY(UFloatProperty,PhysRate);
	REGISTER_MOVER_OBJECT(MyMarker,ANavigationPoint::StaticClass());
	REGISTER_MOVER_OBJECT(TriggerActor,AActor::StaticClass());
	REGISTER_MOVER_OBJECT(TriggerActor2,AActor::StaticClass());
	REGISTER_MOVER_OBJECT(WaitingPawn,APawn::StaticClass());

	const INT MotionBoolOffset = static_cast<INT>(__builtin_offsetof(AMover,RecommendedTrigger) - sizeof(BITFIELD));
	REGISTER_MOVER_BOOL(bOpening,MotionBoolOffset,1);
	REGISTER_MOVER_BOOL(bDelaying,MotionBoolOffset,2);
	REGISTER_MOVER_BOOL(bClientPause,MotionBoolOffset,4);
	REGISTER_MOVER_BOOL(bPlayerOnly,MotionBoolOffset,8);

	REGISTER_MOVER_OBJECT(RecommendedTrigger,ATrigger::StaticClass());
	REGISTER_MOVER_STRUCT(SimOldPos,VectorStruct);
	REGISTER_MOVER_PROPERTY(UIntProperty,SimOldRotPitch);
	REGISTER_MOVER_PROPERTY(UIntProperty,SimOldRotYaw);
	REGISTER_MOVER_PROPERTY(UIntProperty,SimOldRotRoll);
	REGISTER_MOVER_STRUCT(SimInterpolate,VectorStruct);
	REGISTER_MOVER_STRUCT(RealPosition,VectorStruct);
	REGISTER_MOVER_STRUCT(RealRotation,RotatorStruct);
	REGISTER_MOVER_PROPERTY(UIntProperty,ClientUpdate);

	const INT CorralBoolOffset = static_cast<INT>(__builtin_offsetof(AMover,MoverSpringTime) - sizeof(BITFIELD));
	REGISTER_MOVER_BOOL(bCorralMover,CorralBoolOffset,1);
	REGISTER_MOVER_BOOL(bCorraledFlag,CorralBoolOffset,2);

	REGISTER_MOVER_PROPERTY(UFloatProperty,MoverSpringTime);
	REGISTER_MOVER_PROPERTY(UFloatProperty,MoverMaxAmplitude);
	REGISTER_MOVER_PROPERTY(UByteProperty,MoverFluctuations);
	REGISTER_MOVER_STRUCT(HitPosition,VectorStruct);
	REGISTER_MOVER_STRUCT(HitNormal,VectorStruct);

#undef REGISTER_MOVER_BOOL
#undef REGISTER_MOVER_STRUCT_ARRAY
#undef REGISTER_MOVER_STRUCT
#undef REGISTER_MOVER_OBJECT
#undef REGISTER_MOVER_ARRAY_PROPERTY
#undef REGISTER_MOVER_PROPERTY

	unguard;
}
void AMover::Spawned()
{
	guard(AMover::Spawned);
	ABrush::Spawned();

	BasePos = Location;
	BaseRot	= Rotation;

	unguard;
}
void AMover::PostLoad()
{
	guard(AMover::PostLoad);
	AActor::PostLoad();

	// For refresh.
	SavedPos = FVector(-12345,-12345,-12345);
	SavedRot = FRotator(123,456,789);

	// Fix brush poly iLinks which were broken.
	if( Brush && Brush->Polys )
		for( INT i=0; i<Brush->Polys->Element.Num(); i++ )
			Brush->Polys->Element(i).iLink = i;

	unguard;
}
void AMover::PostEditMove()
{
	guard(AMover::PostEditMove);
	ABrush::PostEditMove();
	if( KeyNum == 0 )
	{
		// Changing location.
		BasePos  = Location - OldPos;
		BaseRot  = Rotation - OldRot;
	}
	else
	{
		// Changing displacement of KeyPos[KeyNum] relative to KeyPos[0].
		KeyPos[KeyNum] = Location - (BasePos + KeyPos[0]);
		KeyRot[KeyNum] = Rotation - (BaseRot + KeyRot[0]);

		// Update Old:
		OldPos = KeyPos[KeyNum];
		OldRot = KeyRot[KeyNum];
	}
	Location = BasePos + KeyPos[KeyNum];
	unguard;
}
void AMover::PostEditChange()
{
	guard(AMover::PostEditChange);
	ABrush::PostEditChange();

	// Validate KeyNum.
	KeyNum = Clamp( (INT)KeyNum, (INT)0, (INT)ARRAY_COUNT(KeyPos)-1 );

	// Update BasePos.
	BasePos  = Location - OldPos;
	BaseRot  = Rotation - OldRot;

	// Update Old.
	OldPos = KeyPos[KeyNum];
	OldRot = KeyRot[KeyNum];

	// Update Location.
	Location = BasePos + OldPos;
	Rotation = BaseRot + OldRot;

	PostEditMove();

	unguard;
}
void AMover::PreRaytrace()
{
	guard(AMover::PreRaytrace);
	ABrush::PreRaytrace();

	// Place this brush in position to raytrace the world.
	SavedPos = FVector(0,0,0);
	SavedRot = FRotator(0,0,0);

	unguard;
}
void AMover::SetWorldRaytraceKey()
{
	guard(AMover::SetWorldRaytraceKey);
	if( WorldRaytraceKey!=255 )
	{
		WorldRaytraceKey = Clamp((INT)WorldRaytraceKey,0,(INT)ARRAY_COUNT(KeyPos)-1);
		if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->RemoveActor( this );
		Location = BasePos + KeyPos[WorldRaytraceKey];
		Rotation = BaseRot + KeyRot[WorldRaytraceKey];
		if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->AddActor( this );
		if( GetLevel()->BrushTracker )
			GetLevel()->BrushTracker->Update( this );
	}
	else
	{
		if( GetLevel()->BrushTracker )
			GetLevel()->BrushTracker->Flush( this );
	}
	unguard;
}
void AMover::SetBrushRaytraceKey()
{
	guard(AMover::SetBrushRaytraceKey);

	BrushRaytraceKey = Clamp((INT)BrushRaytraceKey,0,(INT)ARRAY_COUNT(KeyPos)-1);
	if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->RemoveActor( this );
	Location = BasePos + KeyPos[BrushRaytraceKey];
	Rotation = BaseRot + KeyRot[BrushRaytraceKey];
	if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->AddActor( this );
	if( GetLevel()->BrushTracker )
		GetLevel()->BrushTracker->Update( this );

	unguard;
}
void AMover::PostRaytrace()
{
	guard(AMover::PostRaytrace);
	ABrush::PostRaytrace();

	// Called before/after raytracing session beings.
	if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->RemoveActor( this );
	Location = BasePos + KeyPos[KeyNum];
	Rotation = BaseRot + KeyRot[KeyNum];
	if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->AddActor( this );
	SavedPos = FVector(0,0,0);
	SavedRot = FRotator(0,0,0);
	if( GetLevel()->BrushTracker )
		GetLevel()->BrushTracker->Update( this );

	unguard;
}
IMPLEMENT_CLASS(AMover);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
