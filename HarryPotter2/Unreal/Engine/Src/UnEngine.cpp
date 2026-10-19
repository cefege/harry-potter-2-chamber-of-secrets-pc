/*=============================================================================
	UnEngine.cpp: Unreal engine main.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "Hp2UEngineServices.h"


/*-----------------------------------------------------------------------------
	Object class implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UEngine);
IMPLEMENT_CLASS(URenderBase);
IMPLEMENT_CLASS(URenderDevice);
IMPLEMENT_CLASS(URenderIterator);

/*-----------------------------------------------------------------------------
	Engine init and exit.
-----------------------------------------------------------------------------*/

//
// Construct the engine.
//
UEngine::UEngine()
{
	guard(UEngine::UEngine);
	unguard;
}

//
// Init class.
//
void UEngine::StaticConstructor()
{
	guard(UEngine::StaticConstructor);

	new(GetClass(),TEXT("CacheSizeMegs"),      RF_Public)UIntProperty (CPP_PROPERTY(CacheSizeMegs      ), TEXT("Settings"), CPF_Config );
	new(GetClass(),TEXT("UseSound"),           RF_Public)UBoolProperty(CPP_PROPERTY(UseSound           ), TEXT("Settings"), CPF_Config );
	CurrentTickRate = 0.f;

	unguard;
}

// Register things.
#define NAMES_ONLY
#define AUTOGENERATE_NAME(name) ENGINE_API FName ENGINE_##name;
#define AUTOGENERATE_FUNCTION(cls,idx,name) IMPLEMENT_FUNCTION(cls,idx,name)
#include "EngineClasses.h"
#undef AUTOGENERATE_FUNCTION
#undef AUTOGENERATE_NAME
#undef NAMES_ONLY

//
// Init audio.
//
void UEngine::InitAudio()
{
	guard(UEngine::InitAudio);
	hp2_rs_engine_uengine_init_audio(&GHp2UEngineServices, this);
	unguard;
}

//
// Initialize the engine.
//
void UEngine::Init()
{
	guard(UEngine::Init);

	// Add the intrinsic names.
	#define NAMES_ONLY
	#define AUTOGENERATE_NAME(name) ENGINE_##name = FName(TEXT(#name),FNAME_Intrinsic);
	#define AUTOGENERATE_FUNCTION(cls,idx,name)
	#include "EngineClasses.h"
	#undef AUTOGENERATE_FUNCTION
	#undef AUTOGENERATE_NAME
	#undef NAMES_ONLY

	hp2_rs_engine_uengine_init(&GHp2UEngineServices, this);
	unguard;
}

//
// Pre shutdown.
//
void UEngine::Exit()
{
	guard(UEngine::Exit);
	hp2_rs_engine_uengine_exit(&GHp2UEngineServices, this);
	unguard;
}

//
// Exit the engine.
//
void UEngine::Destroy()
{
	guard(UEngine::Destroy);

	hp2_rs_engine_uengine_destroy(&GHp2UEngineServices, this);

	Super::Destroy();
	unguard;
}

//
// Flush all caches.
//
void UEngine::Flush( UBOOL AllowPrecache )
{
	guard(UEngine::Flush);

	hp2_rs_engine_uengine_flush(&GHp2UEngineServices, this, AllowPrecache);

	unguard;
}

//
// Tick rate.
//
FLOAT UEngine::GetMaxTickRate()
{
	guard(UEngine::GetMaxTickRate);
	return hp2_rs_engine_uengine_get_max_tick_rate(&GHp2UEngineServices);
	unguard;
}

//
// Progress indicator.
//
void UEngine::SetProgress( const TCHAR* Str1, const TCHAR* Str2, FLOAT Seconds )
{
	guard(UEngine::SetProgress);
	unguard;
}

//
// Serialize.
//
void UEngine::Serialize( FArchive& Ar )
{
	guard(UGameEngine::Serialize);

	Super::Serialize( Ar );
	Ar << Cylinder << OCylinder << OvalCylinder << OOvalCylinder << Box << Client << Render << Audio;

	unguardobj;
}

/*-----------------------------------------------------------------------------
	Input.
-----------------------------------------------------------------------------*/

//
// This always going to be the last exec handler in the chain. It
// handles passing the command to all other global handlers.
//
UBOOL UEngine::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UEngine::Exec);

	// Global handlers and engine commands are dispatched by the Rust port.
	return hp2_rs_engine_uengine_exec(&GHp2UEngineServices, this, (const uint32_t*)Cmd, &Ar);
	unguard;
}

//
// Key handler.
//
UBOOL UEngine::Key( UViewport* Viewport, EInputKey Key )
{
	guard(UEngine::Key);
	return hp2_rs_engine_uengine_key(&GHp2UEngineServices, Viewport, Key);
	unguard;
}

//
// Input event handler.
//
UBOOL UEngine::InputEvent( UViewport* Viewport, EInputKey iKey, EInputAction State, FLOAT Delta )
{
	guard(UEngine::InputEvent);

	return hp2_rs_engine_uengine_input_event(&GHp2UEngineServices, Viewport, iKey, State, Delta);
	unguard;
}

INT UEngine::ChallengeResponse( INT Challenge )
{
	guard(UEngine::ChallengeResponse);
	return 0;
	unguard;
}

/*-----------------------------------------------------------------------------
	UServerCommandlet.
-----------------------------------------------------------------------------*/

void UServerCommandlet::StaticConstructor()
{
	guard(UServerCommandlet::StaticConstructor);

	LogToStdout = 1;
	IsClient    = 0;
	IsEditor    = 0;
	IsServer    = 1;
	LazyLoad    = 1;

	unguard;
}

INT UServerCommandlet::Main( const TCHAR* Parms )
{
	guard(UServerCommandlet::Main);

	return hp2_rs_engine_uengine_commandlet_main(&GHp2UEngineServices);
	unguard;
}

IMPLEMENT_CLASS(UServerCommandlet)

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
