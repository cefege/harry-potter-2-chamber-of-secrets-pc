/*=============================================================================
	HP2TraceHooks.h: Process-wide diagnostic hook bindings.
=============================================================================*/

#ifndef HP2TRACEHOOKS_H
#define HP2TRACEHOOKS_H

#include "Core.h"

class AActor;
class UClass;
class UFunction;
struct FFrame;
class ADecal;
class UFireTexture;

struct FHP2TraceHookBindings
{
	void (*CreatureDebugInfo)( UObject* Object, FFrame& Stack, const TCHAR* InfoType, INT LineNumber );
	void (*CreatureFunctionEnter)( UObject* Object, UFunction* Function );
	void (*CreatureFunctionExit)( UObject* Object, UFunction* Function, void* Result );
	void (*CreatureRand)( UObject* Object, FFrame& Stack, INT Raw, INT Bound, INT Index );
	void (*CreatureRandRange)( UObject* Object, FFrame& Stack, INT Raw, FLOAT Min, FLOAT Max, FLOAT Value );
	void (*CreatureSoftwareRendering)( AActor* Actor, UBOOL IsSoftwareRendering );
	void (*CreatureTickDispatch)( AActor* Actor, FLOAT DeltaSeconds );
	void (*CreatureClassResolution)( UClass* Class, AActor* Owner );
	void (*CreatureSpawnRequest)( UClass* Class, AActor* Owner );
	void (*CreatureSpawnResult)( AActor* Owner, AActor* Child );
	void (*CreatureSpawnPublished)( AActor* Owner, AActor* Child );

	void (*FireInitTables)( unsigned long long Before, unsigned long long After, unsigned long long Fingerprint );
	void (*FireSpeedRand)( DWORD StateIndex, BYTE Value );
	void (*FireBurnWrite)( UFireTexture* Texture, INT SparkIndex, DWORD Offset, BYTE Before, BYTE Value );

	UBOOL (*GlobalTickEnabled)();
	unsigned long long (*GlobalTickBeginDispatch)( AActor* Actor, const char* Relation, const char* RootAdmission );
	void (*GlobalTickEventTick)( unsigned long long Token );
	void (*GlobalTickProcessState)( AActor* Actor, UObject* State, const char* Outcome );
	void (*GlobalTickEndDispatch)( unsigned long long Token, const char* SkipReason );

	void (*ActorTransitionPostInitExecution)( AActor* Actor );
	void (*ActorTransitionPreBeginBefore)( AActor* Actor );
	void (*ActorTransitionPreBeginAfter)( AActor* Actor );
	void (*ActorTransitionSpawnRequest)( UClass* Class, FName Name, AActor* Owner );
	void (*ActorTransitionSpawnResult)( AActor* Owner, AActor* Child );
	void (*ActorTransitionSpawnPublished)( AActor* Owner, AActor* Child );
	void (*ActorTransitionFirstRendererCandidate)( AActor* Owner, AActor* Candidate );

	INT (*ShadowAdmissionTraceBeginPass)();
	void (*ShadowAdmissionTraceRecord)(
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
		UBOOL WorldDynamics );
};

void HP2InstallTraceHookBindings( const FHP2TraceHookBindings& Bindings );

void HP2CreatureGeneratorTraceDebugInfo( UObject* Object, FFrame& Stack, const TCHAR* InfoType, INT LineNumber );
void HP2CreatureGeneratorTraceFunctionEnter( UObject* Object, UFunction* Function );
void HP2CreatureGeneratorTraceFunctionExit( UObject* Object, UFunction* Function, void* Result );
void HP2CreatureGeneratorTraceRand( UObject* Object, FFrame& Stack, INT Raw, INT Bound, INT Index );
void HP2CreatureGeneratorTraceRandRange( UObject* Object, FFrame& Stack, INT Raw, FLOAT Min, FLOAT Max, FLOAT Value );
void HP2CreatureGeneratorTraceSoftwareRendering( AActor* Actor, UBOOL IsSoftwareRendering );
void HP2CreatureGeneratorTraceTickDispatch( AActor* Actor, FLOAT DeltaSeconds );
void HP2CreatureGeneratorTraceClassResolution( UClass* Class, AActor* Owner );
void HP2CreatureGeneratorTraceSpawnRequest( UClass* Class, AActor* Owner );
void HP2CreatureGeneratorTraceSpawnResult( AActor* Owner, AActor* Child );
void HP2CreatureGeneratorTraceSpawnPublished( AActor* Owner, AActor* Child );

extern "C" void HP2FireTextureTraceInitTables( unsigned long long Before, unsigned long long After, unsigned long long Fingerprint );
extern "C" void HP2FireTextureTraceSpeedRand( DWORD StateIndex, BYTE Value );
extern "C" void HP2FireTextureTraceBurnWrite( UFireTexture* Texture, INT SparkIndex, DWORD Offset, BYTE Before, BYTE Value );

UBOOL HP2GlobalTickTraceEnabled();
unsigned long long HP2GlobalTickTraceBeginDispatch( AActor* Actor, const char* Relation, const char* RootAdmission );
void HP2GlobalTickTraceEventTick( unsigned long long Token );
void HP2GlobalTickTraceProcessState( AActor* Actor, UObject* State, const char* Outcome );
void HP2GlobalTickTraceEndDispatch( unsigned long long Token, const char* SkipReason );

void HP2ActorTransitionPostInitExecution( AActor* Actor );
void HP2ActorTransitionPreBeginBefore( AActor* Actor );
void HP2ActorTransitionPreBeginAfter( AActor* Actor );
void HP2ActorTransitionSpawnRequest( UClass* Class, FName Name, AActor* Owner );
void HP2ActorTransitionSpawnResult( AActor* Owner, AActor* Child );
void HP2ActorTransitionSpawnPublished( AActor* Owner, AActor* Child );
void HP2ActorTransitionFirstRendererCandidate( AActor* Owner, AActor* Candidate );

INT HP2ShadowAdmissionTraceBeginPass();
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
	UBOOL WorldDynamics );

#endif
