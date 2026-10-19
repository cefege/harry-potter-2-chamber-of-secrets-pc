/*=============================================================================
	Hp2ActorServices.cpp: AActor services called by the Rust engine port
	(crates/hp2-engine/src/actor.rs). Each adapter decodes raw pointers into
	the C++ types and forwards to the engine call the original UnActor.cpp body
	made.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "UnMesh.h"
#include "Hp2ActorServices.h"

static_assert( NAME_None==0, "Hp2ActorServices: FName index 0 must be NAME_None" );

static int ActorLevelHasHash( void* Level )
{
	return ((ULevel*)Level)->Hash != NULL;
}

static void ActorLevelHashAddActor( void* Level, void* Actor )
{
	((ULevel*)Level)->Hash->AddActor( (AActor*)Actor );
}

static void ActorLevelHashRemoveActor( void* Level, void* Actor )
{
	((ULevel*)Level)->Hash->RemoveActor( (AActor*)Actor );
}

static int ActorIsA( const void* Obj, unsigned int ClassId )
{
	const UObject* Object = (const UObject*)Obj;
	switch( ClassId )
	{
		case 0: return Object->IsA( AMover::StaticClass() );
		case 1: return Object->IsA( APawn::StaticClass() );
		case 2: return Object->IsA( APlayerPawn::StaticClass() );
		case 3: return Object->IsA( APlayerReplicationInfo::StaticClass() );
		case 4: return Object->IsA( AProjectile::StaticClass() );
		case 5: return Object->IsA( ATrigger::StaticClass() );
		case 6: return Object->IsA( ABrush::StaticClass() );
	}
	return 0;
}

static int ActorLevelFarMoveActor( void* Level, void* Actor, const void* Dest, int Test, int NoCheck )
{
	return ((ULevel*)Level)->FarMoveActor( (AActor*)Actor, *(const FVector*)Dest, Test, NoCheck );
}

static void ActorLevelMoveActorToRotation( void* Level, void* Actor, const void* Rot )
{
	FCheckResult Hit;
	((ULevel*)Level)->MoveActor( (AActor*)Actor, FVector(0,0,0), *(const FRotator*)Rot, Hit, 0, 0, 0, 1 );
}

static int ActorMoveSmooth( void* Actor, const void* Delta )
{
	return ((AActor*)Actor)->moveSmooth( *(const FVector*)Delta );
}

static void ActorSetPhysics( void* Actor, unsigned char Phys )
{
	((AActor*)Actor)->setPhysics( Phys );
}

static void ActorEventBump( void* Actor, void* Other )
{
	((AActor*)Actor)->eventBump( (AActor*)Other );
}

static void ActorEventDetach( void* Base, void* Actor )
{
	((AActor*)Base)->eventDetach( (AActor*)Actor );
}

static void ActorEventAttach( void* Base, void* Actor )
{
	((AActor*)Base)->eventAttach( (AActor*)Actor );
}

static void ActorEventBaseChange( void* Actor )
{
	((AActor*)Actor)->eventBaseChange();
}

static void ActorEventLostChild( void* Owner, void* Actor )
{
	((AActor*)Owner)->eventLostChild( (AActor*)Actor );
}

static void ActorEventGainedChild( void* Owner, void* Actor )
{
	((AActor*)Owner)->eventGainedChild( (AActor*)Actor );
}

static int ActorLevelDemoRecHasServerConnection( void* Level )
{
	return ((ULevel*)Level)->DemoRecDriver->ServerConnection != NULL;
}

static void* ActorLevelViewport0ActorPri( void* Level )
{
	APawn* Actor = ((ULevel*)Level)->Engine->Client->Viewports(0)->Actor;
	return Actor ? Actor->PlayerReplicationInfo : NULL;
}

static float ActorLevelNetAverageFrameTime( void* Level )
{
	return ((ULevel*)Level)->NetDriver->ServerConnection->AverageFrameTime;
}

static void ActorCheckFailed( const char* Expr, int Line )
{
	appFailAssert( Expr, "HarryPotter2/Unreal/Engine/Src/UnActor.cpp", Line );
}

static void ActorEventTouch( void* Actor, void* Other )
{
	((AActor*)Actor)->eventTouch( (AActor*)Other );
}

static void ActorEventUnTouch( void* Actor, void* Other )
{
	((AActor*)Actor)->eventUnTouch( (AActor*)Other );
}

static int ActorIsEditor( void )
{
	return GIsEditor != 0;
}

static void ActorGCacheFlush( void )
{
	GCache.Flush();
}

static void* ActorLevelEnginePrimitive( void* Level, unsigned char Slot )
{
	ULevel* L = (ULevel*)Level;
	switch( Slot )
	{
		default:
		case 0:
			checkSlow( L->Engine->Cylinder );
			return L->Engine->Cylinder;
		case 1:
			checkSlow( L->Engine->OCylinder );
			return L->Engine->OCylinder;
		case 2:
			checkSlow( L->Engine->Box );
			return L->Engine->Box;
		case 4:
			checkSlow( L->Engine->OvalCylinder );
			return L->Engine->OvalCylinder;
		case 5:
			checkSlow( L->Engine->OOvalCylinder );
			return L->Engine->OOvalCylinder;
	}
}

static void ActorPrimitiveValidateActor( void* Prim, void* Actor )
{
	((UPrimitive*)Prim)->ValidateActor( (AActor*)Actor );
}

static int ActorLevelBegunPlay( void* LevelInfo )
{
	return ((ALevelInfo*)LevelInfo)->bBegunPlay != 0;
}

static void* ActorGetPrimitive( void* Actor )
{
	return ((AActor*)Actor)->GetPrimitive();
}

static int ActorObjectIsValid( void* Obj )
{
	return ((UObject*)Obj)->IsValid();
}

static void ActorUObjectProcessEvent( void* Obj, void* Func, void* Parms, void* Result )
{
	((UObject*)Obj)->UObject::ProcessEvent( (UFunction*)Func, Parms, Result );
}

static void ActorUObjectDestroy( void* Obj )
{
	((UObject*)Obj)->UObject::Destroy();
}

static void ActorRenderInterfaceDestroy( void* RI )
{
	URenderIterator* R = (URenderIterator*)RI;
	R->RemoveFromRoot();
	R->ConditionalDestroy();
	delete R;
}

static void ActorObjectConditionalDestroy( void* Obj )
{
	((UObject*)Obj)->ConditionalDestroy();
}

static void ActorObjectSetTransactional( void* Obj )
{
	((UObject*)Obj)->SetFlags( RF_Transactional );
}

static int ActorClassIsLocalized( void* Actor )
{
	return (((AActor*)Actor)->GetClass()->ClassFlags & CLASS_Localized) != 0;
}

static void ActorLoadLocalized( void* Actor )
{
	((AActor*)Actor)->LoadLocalized();
}

static void ActorUObjectPostLoad( void* Obj )
{
	((UObject*)Obj)->UObject::PostLoad();
}

static void* ActorModelPolys( void* Model )
{
	return ((UModel*)Model)->Polys;
}

static void* ActorPlayerActor( void* Player )
{
	return ((UPlayer*)Player)->Actor;
}

static void ActorPlayerSetActor( void* Player, void* Actor )
{
	((UPlayer*)Player)->Actor = (APlayerPawn*)Actor;
}

static void ActorEventPossess( void* Pawn )
{
	((APlayerPawn*)Pawn)->eventPossess();
}

static void ActorLogPossessed( void* Pawn )
{
	debugf( NAME_Log, TEXT("Possessed PlayerPawn: %s"), ((APlayerPawn*)Pawn)->GetFullName() );
}

static void ActorMeshCollisionExtent( void* Mesh, void* Actor, void* Out )
{
	*(FVector*)Out = ((UMesh*)Mesh)->GetCollisionExtent( (AActor*)Actor );
}

static void ActorBrushCollisionExtent( void* Brush, void* Actor, void* Out )
{
	*(FVector*)Out = ((UModel*)Brush)->GetCollisionExtent( (AActor*)Actor );
}

static void ActorMeshRenderBoundingBox( void* Mesh, void* Actor, int Exact, void* Out )
{
	*(FCoords*)Out = ((UMesh*)Mesh)->GetRenderBoundingBox( (AActor*)Actor, Exact );
}

static void ActorBrushRenderBoundingBoxCoords( void* Brush, void* Actor, int Exact, void* Out )
{
	*(FCoords*)Out = ((UModel*)Brush)->GetRenderBoundingBox( (AActor*)Actor, Exact ).GetCoords();
}

static void ActorObjectSetDebugSerialize( void* Obj )
{
	((UObject*)Obj)->SetFlags( RF_DebugSerialize );
}

static void* ActorObjectClass( void* Obj )
{
	return ((UObject*)Obj)->GetClass();
}

static int ActorClassIsUClassStatic( void* Class )
{
	return (UClass*)Class == UClass::StaticClass();
}

static void ActorArchivePreload( void* Ar, void* Obj )
{
	((FArchive*)Ar)->Preload( (UObject*)Obj );
}

static int ActorArchiveIsLoading( void* Ar )
{
	return ((FArchive*)Ar)->IsLoading() != 0;
}

static int ActorArchiveIsSaving( void* Ar )
{
	return ((FArchive*)Ar)->IsSaving() != 0;
}

static int ActorArchiveIsTrans( void* Ar )
{
	return ((FArchive*)Ar)->IsTrans() != 0;
}

static void ActorObjectSerializeTaggedPersistent( void* Obj, void* Ar )
{
	UObject* O = (UObject*)Obj;
	O->GetClass()->SerializeTaggedPersistentProperties( *(FArchive*)Ar, (BYTE*)Obj, O->GetClass() );
}

static void ActorObjectSerializeBin( void* Obj, void* Ar )
{
	((UObject*)Obj)->GetClass()->SerializeBin( *(FArchive*)Ar, (BYTE*)Obj );
}

static size_t ActorObjectPropertiesSize( void* Obj )
{
	return ((UObject*)Obj)->GetClass()->GetPropertiesSize();
}

static void ActorArchiveCountBytes( void* Ar, size_t Size )
{
	((FArchive*)Ar)->CountBytes( Size, Size );
}

static void* ActorTraceLineTextureHit( void* Actor, const void* TraceEnd, const void* TraceStart, void* OutLocation )
{
	AActor* A = (AActor*)Actor;
	FCheckResult Hit(1.0);
	A->GetLevel()->SingleLineCheck( Hit, A, *(const FVector*)TraceEnd, *(const FVector*)TraceStart, TRACE_Level, FVector(0, 0, 0) );
	*(FVector*)OutLocation = Hit.Location;
	return (void*)Hit.BspSurf();
}

static void* ActorXLevelModel( void* Actor )
{
	return ((AActor*)Actor)->XLevel->Model;
}

static void* BspSurfTexture( void* Surf )
{
	return ((FBspSurf*)Surf)->Texture;
}

static unsigned int BspSurfPolyFlags( void* Surf )
{
	return ((FBspSurf*)Surf)->PolyFlags;
}

static unsigned int TexturePolyFlags( void* Texture )
{
	return ((UTexture*)Texture)->PolyFlags;
}

static int BspSurfDecalCount( void* Surf )
{
	return ((FBspSurf*)Surf)->Decals.Num();
}

static int BspSurfVNormal( void* Surf )
{
	return ((FBspSurf*)Surf)->vNormal;
}

static int BspSurfPBase( void* Surf )
{
	return ((FBspSurf*)Surf)->pBase;
}

static void ModelVector( void* Model, int Index, void* Out )
{
	*(FVector*)Out = ((UModel*)Model)->Vectors( Index );
}

static void ModelPoint( void* Model, int Index, void* Out )
{
	*(FVector*)Out = ((UModel*)Model)->Points( Index );
}

static void* BspSurfDecal( void* Surf, int Index )
{
	return (void*)&((FBspSurf*)Surf)->Decals( Index );
}

static void DecalVertices( void* Decal, void* Out )
{
	FDecal* D = (FDecal*)Decal;
	for( int i = 0; i < 4; i++ )
		((FVector*)Out)[i] = D->Vertices[i];
}

static void* DecalActorTexture( void* Decal )
{
	return ((FDecal*)Decal)->Actor->Texture;
}

const Hp2ActorServices GHp2ActorServices =
{
	ActorLevelHasHash,
	ActorLevelHashAddActor,
	ActorLevelHashRemoveActor,
	ActorIsA,
	ActorLevelFarMoveActor,
	ActorLevelMoveActorToRotation,
	ActorMoveSmooth,
	ActorSetPhysics,
	ActorEventBump,
	ActorEventDetach,
	ActorEventAttach,
	ActorEventBaseChange,
	ActorEventLostChild,
	ActorEventGainedChild,
	ActorLevelDemoRecHasServerConnection,
	ActorLevelViewport0ActorPri,
	ActorLevelNetAverageFrameTime,
	ActorCheckFailed,
	ActorEventTouch,
	ActorEventUnTouch,
	ActorIsEditor,
	ActorGCacheFlush,
	ActorLevelEnginePrimitive,
	ActorPrimitiveValidateActor,
	ActorLevelBegunPlay,
	ActorUObjectProcessEvent,
	ActorUObjectDestroy,
	ActorRenderInterfaceDestroy,
	ActorObjectConditionalDestroy,
	ActorObjectSetTransactional,
	ActorClassIsLocalized,
	ActorLoadLocalized,
	ActorUObjectPostLoad,
	ActorModelPolys,
	ActorPlayerActor,
	ActorPlayerSetActor,
	ActorEventPossess,
	ActorLogPossessed,
	ActorMeshCollisionExtent,
	ActorBrushCollisionExtent,
	ActorMeshRenderBoundingBox,
	ActorBrushRenderBoundingBoxCoords,
	ActorObjectSetDebugSerialize,
	ActorObjectClass,
	ActorClassIsUClassStatic,
	ActorArchivePreload,
	ActorArchiveIsLoading,
	ActorArchiveIsSaving,
	ActorArchiveIsTrans,
	ActorObjectSerializeTaggedPersistent,
	ActorObjectSerializeBin,
	ActorObjectPropertiesSize,
	ActorArchiveCountBytes,
	ActorTraceLineTextureHit,
	ActorXLevelModel,
	BspSurfTexture,
	BspSurfPolyFlags,
	TexturePolyFlags,
	BspSurfDecalCount,
	BspSurfVNormal,
	BspSurfPBase,
	ModelVector,
	ModelPoint,
	BspSurfDecal,
	DecalVertices,
	DecalActorTexture,
	ActorGetPrimitive,
	ActorObjectIsValid,
};
