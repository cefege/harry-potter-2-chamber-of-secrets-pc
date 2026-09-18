/*=============================================================================
	HP2ActorSlotDumpHooks.cpp: inert defaults for optional launch diagnostics.
=============================================================================*/

#include "Engine.h"

#if defined(__GNUC__)
#define HP2_WEAK_HOOK __attribute__((weak))
#else
#define HP2_WEAK_HOOK
#endif

void HP2_WEAK_HOOK HP2ActorSlotDumpPostDeserialize( ULevel* Level )
{
}

void HP2_WEAK_HOOK HP2ActorSlotDumpBeginRawLevelActorTable( ULevelBase* Level )
{
}

void HP2_WEAK_HOOK HP2ActorSlotDumpRawLevelActor(
	ULevelBase* Level,
	INT Slot,
	AActor* Actor,
	INT ReferenceOffsetBefore,
	INT ReferenceOffsetAfter,
	INT CompactPackageIndex,
	UBOOL CompactPackageIndexCaptured,
	UBOOL CompactPackageIndexSupported )
{
}

void HP2_WEAK_HOOK HP2ActorSlotDumpEndRawLevelActorTable( ULevelBase* Level )
{
}

#undef HP2_WEAK_HOOK
