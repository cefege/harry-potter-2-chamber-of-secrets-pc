/*=============================================================================
	UnLevel.cpp: Level-related functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "Hp2LevelServices.h"
#include "Hp2FfiException.h"

#include <cstdint>


/*-----------------------------------------------------------------------------
	ULevelBase implementation.
-----------------------------------------------------------------------------*/

ULevelBase::ULevelBase( UEngine* InEngine, const FURL& InURL )
:	URL( InURL )
,	Engine( InEngine )
,	Actors( this )
,	DemoRecDriver( NULL )
{}

void ULevelBase::Serialize( FArchive& Ar )
{
	guard(ULevelBase::Serialize);
	hp2_rs_engine_level_base_serialize( &GHp2LevelCoreServices, this, &Ar );
	unguard;
}
void ULevelBase::Destroy()
{
	guard(ULevelBase::Destroy);
	hp2_rs_engine_level_base_destroy( &GHp2LevelCoreServices, this );
	unguard;
}
void ULevelBase::NotifyProgress( const TCHAR* Str1, const TCHAR* Str2, FLOAT Seconds )
{
	guard(ULevelBase::NotifyProgress);
	Engine->SetProgress( Str1, Str2, Seconds );
	unguard;
}
IMPLEMENT_CLASS(ULevelBase);

/*-----------------------------------------------------------------------------
	Level creation & emptying.
-----------------------------------------------------------------------------*/

//
//	Create a new level and allocate all objects needed for it.
//	Call with Editor=1 to allocate editor structures for it, also.
//
ULevel::ULevel( UEngine* InEngine, UBOOL InRootOutside )
:	ULevelBase( InEngine )
{
	guard(ULevel::ULevel);
	Hp2RethrowPending( hp2_rs_engine_level_init( &GHp2LevelCoreServices, this, InRootOutside ) );
	unguard;
}


// *******************************************************************
//
//	HP2 Specific level functions
//
// *******************************************************************

void ULevel::ScreenActorsByGameState( void )
{
	hp2_rs_engine_level_screen_actors_by_game_state( &GHp2LevelCoreServices, this );
}

bool ULevel::SavePersistentActors( const FString& MapName )
{
	return hp2_rs_engine_level_save_persistent_actors( &GHp2LevelPersistServices, this, reinterpret_cast<const uint32_t*>(*MapName) ) != 0;
}


bool ULevel::LoadPersistentActors( const FString& MapName )
{
	return hp2_rs_engine_level_load_persistent_actors( &GHp2LevelPersistServices, this, reinterpret_cast<const uint32_t*>(*MapName) ) != 0;
}




void ULevel::ShrinkLevel()
{
	guard(ULevel::Shrink);
	hp2_rs_engine_level_shrink( &GHp2LevelCoreServices, this );
	unguard;
}
void ULevel::DetailChange( UBOOL NewDetail )
{
	guard(ULevel::DetailChange);
	hp2_rs_engine_level_detail_change( &GHp2LevelCoreServices, this, NewDetail );
	unguard;
}

/*-----------------------------------------------------------------------------
	Level locking and unlocking.
-----------------------------------------------------------------------------*/

//
// Modify this level.
//
void ULevel::Modify( UBOOL DoTransArrays )
{
	guard(ULevel::Modify);
	hp2_rs_engine_level_modify( &GHp2LevelCoreServices, this );
	unguard;
}
void ULevel::PostLoad()
{
	guard(ULevel::PostLoad);
	Hp2RethrowPending( hp2_rs_engine_level_post_load( &GHp2LevelCoreServices, this ) );
	unguard;
}
void ULevel::SetActorCollision( UBOOL bCollision )
{
	guard(ULevel::SetActorCollision);
	hp2_rs_engine_level_set_actor_collision( &GHp2LevelCoreServices, this, bCollision );
	unguard;
}

/*-----------------------------------------------------------------------------
	Level object implementation.
-----------------------------------------------------------------------------*/

void ULevel::Serialize( FArchive& Ar )
{
	guard(ULevel::Serialize);
	hp2_rs_engine_level_serialize( &GHp2LevelCoreServices, this, &Ar );
	unguard;
}


void ULevel::Destroy()
{
	guard(ULevel::Destroy);
	hp2_rs_engine_level_destroy( &GHp2LevelCoreServices, this );
	unguard;
}
IMPLEMENT_CLASS(ULevel);

/*-----------------------------------------------------------------------------
	Reconcile actors and Viewports after loading or creating a new level.

	These functions provide the basic mechanism by which UnrealEd associates
	Viewports and actors together, even when new maps are loaded which contain
	an entirely different set of actors which must be mapped onto the existing 
	Viewports.
-----------------------------------------------------------------------------*/

//
// Remember actors.
//
void ULevel::RememberActors()
{
	guard(ULevel::RememberActors);
	hp2_rs_engine_level_remember_actors( &GHp2LevelCoreServices, this );
	unguard;
}

//
// Reconcile actors.  This is called after loading a level.
// It attempts to match each existing Viewport to an actor in the newly-loaded
// level.  If no decent match can be found, creates a new actor for the Viewport.
//
void ULevel::ReconcileActors()
{
	guard(ULevel::ReconcileActors);
	Hp2RethrowPending( hp2_rs_engine_level_reconcile_actors( &GHp2LevelCoreServices, this ) );
	unguard;
}

/*-----------------------------------------------------------------------------
	ULevel command-line.
-----------------------------------------------------------------------------*/

UBOOL ULevel::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(ULevel::Exec);
	return hp2_rs_engine_level_exec( &GHp2LevelNetServices, this, reinterpret_cast<const uint32_t*>(Cmd), &Ar );
	unguard;
}

/*-----------------------------------------------------------------------------
	ULevel networking related functions.
-----------------------------------------------------------------------------*/

//
// Start listening for connections.
//
UBOOL ULevel::Listen( FString& Error )
{
	guard(ULevel::Listen);
	return hp2_rs_engine_level_listen( &GHp2LevelNetServices, this, &Error );
	unguard;
}

//
// Return whether this level is a server.
//
UBOOL ULevel::IsServer()
{
	guardSlow(ULevel::IsServer);
	return hp2_rs_engine_level_is_server( &GHp2LevelNetServices, this );
	unguardSlow;
}

/*-----------------------------------------------------------------------------
	ULevel network notifys.
-----------------------------------------------------------------------------*/

//
// The network driver is about to accept a new connection attempt by a
// connectee, and we can accept it or refuse it.
//
EAcceptConnection ULevel::NotifyAcceptingConnection()
{
	guard(ULevel::NotifyAcceptingConnection);
	INT Result = 0;
	INT Status = hp2_rs_engine_level_notify_accepting_connection( &GHp2LevelNetServices, this, &Result );
	Hp2RethrowPending( Status );
	return (EAcceptConnection)Result;
	unguard;
}

//
// This server has accepted a connection.
//
void ULevel::NotifyAcceptedConnection( UNetConnection* Connection )
{
	guard(ULevel::NotifyAcceptedConnection);
	INT Status = hp2_rs_engine_level_notify_accepted_connection( &GHp2LevelNetServices, this, Connection );
	Hp2RethrowPending( Status );
	unguard;
}

//
// The network interface is notifying this level of a new channel-open
// attempt by a connectee, and we can accept or refuse it.
//
UBOOL ULevel::NotifyAcceptingChannel( UChannel* Channel )
{
	guard(ULevel::NotifyAcceptingChannel);
	INT Result = 0;
	INT Status = hp2_rs_engine_level_notify_accepting_channel( &GHp2LevelNetServices, this, Channel, &Result );
	Hp2RethrowPending( Status );
	return Result;
	unguard;
}

//
// Welcome a new player joining this server.
//
void ULevel::WelcomePlayer( UNetConnection* Connection, const TCHAR* Optional )
{
	guard(ULevel::WelcomePlayer);
	hp2_rs_engine_level_welcome_player( &GHp2LevelNetServices, this, Connection, reinterpret_cast<const uint32_t*>(Optional) );
	unguard;
}

//
// Received text on the control channel.
//
void ULevel::NotifyReceivedText( UNetConnection* Connection, const TCHAR* Text )
{
	guard(ULevel::NotifyReceivedText);
	hp2_rs_engine_level_notify_received_text( &GHp2LevelNetServices, this, Connection, reinterpret_cast<const uint32_t*>(Text) );
	unguard;
}

//
// Called when a file receive is about to begin.
//
void ULevel::NotifyReceivedFile( UNetConnection* Connection, INT PackageIndex, const TCHAR* Error, UBOOL Skipped )
{
	guard(ULevel::NotifyReceivingFile);
	Hp2RethrowPending( hp2_rs_engine_level_notify_received_file( &GHp2LevelNetServices ) );
	unguard;
}

//
// Called when other side requests a file.
//
UBOOL ULevel::NotifySendingFile( UNetConnection* Connection, FGuid Guid )
{
	guard(ULevel::NotifySendingFile);
	return hp2_rs_engine_level_notify_sending_file( &GHp2LevelNetServices, this );
	unguard;
}

/*-----------------------------------------------------------------------------
	Stats.
-----------------------------------------------------------------------------*/

void ULevel::InitStats()
{
	guard(ULevel::InitStats);
	hp2_rs_engine_level_init_stats( &GHp2LevelCoreServices, this );
	unguard;
}
void ULevel::GetStats( TCHAR* Result )
{
	guard(ULevel::GetStats);
	hp2_rs_engine_level_get_stats( &GHp2LevelCoreServices, this, reinterpret_cast<uint32_t*>(Result) );
	unguard;
}

/*-----------------------------------------------------------------------------
	Clock.
-----------------------------------------------------------------------------*/

void ULevel::UpdateTime(ALevelInfo* Info)
{
	hp2_rs_engine_level_update_time( &GHp2LevelCoreServices, Info );
}

/*-----------------------------------------------------------------------------
	Sound Occlusion
-----------------------------------------------------------------------------*/

UBOOL ULevel::IsAudibleAt( FVector SoundLocation, FVector ListenerLocation, AActor* SoundActor, ESoundOcclusion SoundOcclusion )
{
	guard(ULevel::IsAudibleAt);
	FCheckResult Hit;
	return hp2_rs_engine_level_is_audible_at( &GHp2LevelCoreServices, this, &SoundLocation, &ListenerLocation, SoundActor, SoundOcclusion, &Hit );
	unguard;
}

FLOAT ULevel::CalculateRadiusMultiplier( INT Zone1, INT Zone2 )
{
	guard(AActor::CalculateRadiusMultiplier);
	return hp2_rs_engine_level_calculate_radius_multiplier( this, Zone1, Zone2 );
	unguard;
}

/*-----------------------------------------------------------------------------
	Actors relevant to a viewer.
-----------------------------------------------------------------------------*/

void ULevel::TraceVisible
(
	FVector&		vTraceDirection,
	FCheckResult&	Hit,			// Item hit.
	AActor*			SourceActor,	// Source actor, this or its parents is never hit.
	const FVector&	Start,			// Start location.
	DWORD           TraceFlags,		// Trace flags.
	int				iDistance
)
{
	guard(ULevel::TraceVisible);
	FCheckResult	FirstHit;
	hp2_rs_engine_level_trace_visible( &GHp2LevelCoreServices, this, &vTraceDirection, &Hit, SourceActor, &Start, TraceFlags, iDistance, &FirstHit );
	unguard;
}

// Trace a line and return the first actor hit in the selected section that matches the ParentClass
void ULevel::TraceVisibleObjects
(
	UClass*			ParentClass,	
	FVector&		vTraceDirection,
	FCheckResult&	Hit,			// Item hit.
	AActor*			SourceActor,	// Source actor, this or its parents is never hit.
	const FVector&	Start,			// Start location.
	DWORD           TraceFlags,		// Trace flags.
	int				iDistance
)
{
	guard(ULevel::TraceVisibleObjects);
	FCheckResult	FirstHit;
	hp2_rs_engine_level_trace_visible_objects( &GHp2LevelCoreServices, this, ParentClass, &vTraceDirection, &Hit, SourceActor, &Start, TraceFlags, iDistance, &FirstHit );
	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
