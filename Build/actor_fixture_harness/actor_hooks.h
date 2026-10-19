// Engine-boundary hooks. gen_actor.sh rewrites UnActor.cpp (git archive 90d1e8d)
// so that every virtual, inline-event, class-test, or out-of-TU engine call made
// by the families under test goes through one of these functions. Each hook is
// defined in actor_harness.cpp and prints one "hook ..." line. Pointers are
// printed by registered name, never by address, so logs are byte-stable.
#pragma once


// Arguments for the execTraceTexture P_GET_* replacements.
extern FVector ActorArgTraceEnd;
extern FVector ActorArgTraceStart;
extern INT*    ActorArgFlags;
extern UBOOL   ActorArgDecals;

// Class tests: IsA(X::StaticClass()) and Brush-based IsBrush().
UBOOL ActorHookIsA(const void* obj, const char* cls);
UBOOL ActorHookIsBrush(const void* obj);

// Event dispatch (eventX(...) inline wrappers over ProcessEvent).
void ActorHookEvent(void* obj, const char* name, void* arg);

// Object-level virtuals / non-inline members reached from the families.
void ActorHookSetFlags(void* obj, DWORD flags);
void ActorHookConditionalDestroy(void* obj);
UBOOL ActorHookIsValid(void* obj);
void ActorHookRemoveFromRoot(void* obj);
void ActorHookDeleteRI(void* obj);
void ActorHookLoadLocalized(void* obj);
void ActorHookValidate(void* prim, void* actor);
void ActorHookHash(void* actor, const char* op);
void ActorHookSuperProcessEvent(void* obj, void* func, void* parms, void* result);
void ActorHookPossessed(void* pawn);
void ActorHookCacheFlush();

// Geometry (virtual UPrimitive / UMesh dispatch, FBox::GetCoords chain).
FVector ActorHookCollisionExtent(void* prim, void* actor);
FCoords ActorHookMeshRenderBox(void* mesh, void* actor, UBOOL exact);
FCoords ActorHookBrushRenderBox(void* brush, void* actor, UBOOL exact);
void ActorHookSingleLineCheck(void* level, FCheckResult& hit, void* src,
                              const FVector& end, const FVector& start,
                              DWORD flags, FVector extent);
const FBspSurf* ActorHookBspSurf(const FCheckResult& hit);

// Serialization (FArchive / UClass virtuals).
UClass* ActorHookUClassStatic();
UBOOL ActorHookIsLoading(void* ar);
UBOOL ActorHookIsSaving(void* ar);
UBOOL ActorHookIsTrans(void* ar);
void ActorHookPreload(void* ar, void* cls);
void ActorHookCountBytes(void* ar, SIZE_T n);
void ActorHookSerializeTagged(void* cls, void* ar, void* obj);
void ActorHookSerializeBin(void* cls, void* ar, void* obj);
SIZE_T ActorHookPropertiesSize(void* cls);
void ActorHookFarMove(void* level, AActor* actor, FVector dest, INT test, INT no_check);
void ActorHookMoveActor(void* level, AActor* actor, FVector delta, FRotator rot,
                        FCheckResult& hit, INT encroacher, INT wall, INT grav,
                        INT no_check);
INT ActorHookMoveSmooth(AActor* actor, FVector delta);
void ActorHookSetPhysics(AActor* actor, BYTE physics);
UBOOL ActorHookCanCast(const void* obj, const char* cls);
template<class T> T* ActorHookCast(void* obj, const char* cls)
{
	return ActorHookCanCast(obj, cls) ? reinterpret_cast<T*>(obj) : NULL;
}
