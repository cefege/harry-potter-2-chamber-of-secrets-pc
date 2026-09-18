/*=============================================================================
	HP2TraceHooks.cpp: Process-wide diagnostic hook ownership.
=============================================================================*/

#include "HP2TraceHooks.h"

namespace
{
	FHP2TraceHookBindings GHP2TraceHooks = {};
}

void HP2InstallTraceHookBindings( const FHP2TraceHookBindings& Bindings )
{
	GHP2TraceHooks = Bindings;
}

extern "C" void HP2FireTextureTraceInitTables( unsigned long long Before, unsigned long long After, unsigned long long Fingerprint )
{
	if( GHP2TraceHooks.FireInitTables )
		GHP2TraceHooks.FireInitTables( Before, After, Fingerprint );
}

extern "C" void HP2FireTextureTraceSpeedRand( DWORD StateIndex, BYTE Value )
{
	if( GHP2TraceHooks.FireSpeedRand )
		GHP2TraceHooks.FireSpeedRand( StateIndex, Value );
}

extern "C" void HP2FireTextureTraceBurnWrite( UFireTexture* Texture, INT SparkIndex, DWORD Offset, BYTE Before, BYTE Value )
{
	if( GHP2TraceHooks.FireBurnWrite )
		GHP2TraceHooks.FireBurnWrite( Texture, SparkIndex, Offset, Before, Value );
}

void HP2CreatureGeneratorTraceDebugInfo( UObject* Object, FFrame& Stack, const TCHAR* InfoType, INT LineNumber )
{
	if( GHP2TraceHooks.CreatureDebugInfo )
		GHP2TraceHooks.CreatureDebugInfo( Object, Stack, InfoType, LineNumber );
}

void HP2CreatureGeneratorTraceFunctionEnter( UObject* Object, UFunction* Function )
{
	if( GHP2TraceHooks.CreatureFunctionEnter )
		GHP2TraceHooks.CreatureFunctionEnter( Object, Function );
}

void HP2CreatureGeneratorTraceFunctionExit( UObject* Object, UFunction* Function, void* Result )
{
	if( GHP2TraceHooks.CreatureFunctionExit )
		GHP2TraceHooks.CreatureFunctionExit( Object, Function, Result );
}

void HP2CreatureGeneratorTraceRand( UObject* Object, FFrame& Stack, INT Raw, INT Bound, INT Index )
{
	if( GHP2TraceHooks.CreatureRand )
		GHP2TraceHooks.CreatureRand( Object, Stack, Raw, Bound, Index );
}

void HP2CreatureGeneratorTraceRandRange( UObject* Object, FFrame& Stack, INT Raw, FLOAT Min, FLOAT Max, FLOAT Value )
{
	if( GHP2TraceHooks.CreatureRandRange )
		GHP2TraceHooks.CreatureRandRange( Object, Stack, Raw, Min, Max, Value );
}

void HP2CreatureGeneratorTraceSoftwareRendering( AActor* Actor, UBOOL IsSoftwareRendering )
{
	if( GHP2TraceHooks.CreatureSoftwareRendering )
		GHP2TraceHooks.CreatureSoftwareRendering( Actor, IsSoftwareRendering );
}

void HP2CreatureGeneratorTraceTickDispatch( AActor* Actor, FLOAT DeltaSeconds )
{
	if( GHP2TraceHooks.CreatureTickDispatch )
		GHP2TraceHooks.CreatureTickDispatch( Actor, DeltaSeconds );
}

void HP2CreatureGeneratorTraceClassResolution( UClass* Class, AActor* Owner )
{
	if( GHP2TraceHooks.CreatureClassResolution )
		GHP2TraceHooks.CreatureClassResolution( Class, Owner );
}

void HP2CreatureGeneratorTraceSpawnRequest( UClass* Class, AActor* Owner )
{
	if( GHP2TraceHooks.CreatureSpawnRequest )
		GHP2TraceHooks.CreatureSpawnRequest( Class, Owner );
}

void HP2CreatureGeneratorTraceSpawnResult( AActor* Owner, AActor* Child )
{
	if( GHP2TraceHooks.CreatureSpawnResult )
		GHP2TraceHooks.CreatureSpawnResult( Owner, Child );
}

void HP2CreatureGeneratorTraceSpawnPublished( AActor* Owner, AActor* Child )
{
	if( GHP2TraceHooks.CreatureSpawnPublished )
		GHP2TraceHooks.CreatureSpawnPublished( Owner, Child );
}

UBOOL HP2GlobalTickTraceEnabled()
{
	return GHP2TraceHooks.GlobalTickEnabled ? GHP2TraceHooks.GlobalTickEnabled() : 0;
}

unsigned long long HP2GlobalTickTraceBeginDispatch( AActor* Actor, const char* Relation, const char* RootAdmission )
{
	return GHP2TraceHooks.GlobalTickBeginDispatch
		? GHP2TraceHooks.GlobalTickBeginDispatch( Actor, Relation, RootAdmission )
		: ~0ULL;
}

void HP2GlobalTickTraceEventTick( unsigned long long Token )
{
	if( GHP2TraceHooks.GlobalTickEventTick )
		GHP2TraceHooks.GlobalTickEventTick( Token );
}

void HP2GlobalTickTraceProcessState( AActor* Actor, UObject* State, const char* Outcome )
{
	if( GHP2TraceHooks.GlobalTickProcessState )
		GHP2TraceHooks.GlobalTickProcessState( Actor, State, Outcome );
}

void HP2GlobalTickTraceEndDispatch( unsigned long long Token, const char* SkipReason )
{
	if( GHP2TraceHooks.GlobalTickEndDispatch )
		GHP2TraceHooks.GlobalTickEndDispatch( Token, SkipReason );
}

void HP2ActorTransitionPostInitExecution( AActor* Actor )
{
	if( GHP2TraceHooks.ActorTransitionPostInitExecution )
		GHP2TraceHooks.ActorTransitionPostInitExecution( Actor );
}

void HP2ActorTransitionPreBeginBefore( AActor* Actor )
{
	if( GHP2TraceHooks.ActorTransitionPreBeginBefore )
		GHP2TraceHooks.ActorTransitionPreBeginBefore( Actor );
}

void HP2ActorTransitionPreBeginAfter( AActor* Actor )
{
	if( GHP2TraceHooks.ActorTransitionPreBeginAfter )
		GHP2TraceHooks.ActorTransitionPreBeginAfter( Actor );
}

void HP2ActorTransitionSpawnRequest( UClass* Class, FName Name, AActor* Owner )
{
	if( GHP2TraceHooks.ActorTransitionSpawnRequest )
		GHP2TraceHooks.ActorTransitionSpawnRequest( Class, Name, Owner );
}

void HP2ActorTransitionSpawnResult( AActor* Owner, AActor* Child )
{
	if( GHP2TraceHooks.ActorTransitionSpawnResult )
		GHP2TraceHooks.ActorTransitionSpawnResult( Owner, Child );
}

void HP2ActorTransitionSpawnPublished( AActor* Owner, AActor* Child )
{
	if( GHP2TraceHooks.ActorTransitionSpawnPublished )
		GHP2TraceHooks.ActorTransitionSpawnPublished( Owner, Child );
}

void HP2ActorTransitionFirstRendererCandidate( AActor* Owner, AActor* Candidate )
{
	if( GHP2TraceHooks.ActorTransitionFirstRendererCandidate )
		GHP2TraceHooks.ActorTransitionFirstRendererCandidate( Owner, Candidate );
}

INT HP2ShadowAdmissionTraceBeginPass()
{
	return GHP2TraceHooks.ShadowAdmissionTraceBeginPass
		? GHP2TraceHooks.ShadowAdmissionTraceBeginPass()
		: 0;
}

void HP2ShadowAdmissionTraceRecord(
	INT Pass,
	AActor* Owner,
	ADecal* Shadow,
	const char* CandidateStatus,
	UBOOL UpdateEligible,
	AActor* ViewportActor,
	AActor* ViewTarget,
	UBOOL BehindView,
	AActor* RecursionParentActor,
	UBOOL Perspective,
	UBOOL WorldDynamics )
{
	if( GHP2TraceHooks.ShadowAdmissionTraceRecord )
	{
		GHP2TraceHooks.ShadowAdmissionTraceRecord(
			Pass,
			Owner,
			Shadow,
			CandidateStatus,
			UpdateEligible,
			ViewportActor,
			ViewTarget,
			BehindView,
			RecursionParentActor,
			Perspective,
			WorldDynamics );
	}
}
