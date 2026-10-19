/*=============================================================================
	UnIn.cpp: Unreal input system.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2InputServices.h"

/*-----------------------------------------------------------------------------
	Implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UInput);

/*-----------------------------------------------------------------------------
	UInput creation and destruction.
-----------------------------------------------------------------------------*/

//
// Temporary.
// !!should be moved to an UnrealScript definition.
//
void UInput::StaticInitInput()
{
	guard(UInput::StaticInitInput);
	FArchive ArDummy;

	// Create input alias struct.
	UStruct* AliasStruct = new(StaticClass(),TEXT("Alias"))UStruct( NULL );
	AliasStruct->SetPropertiesSize( sizeof(FAlias) );
	new(AliasStruct,TEXT("Alias"),  RF_Public)UNameProperty( EC_CppProperty, static_cast<INT>(offsetof(FAlias,Alias)),   TEXT(""), CPF_Config );
	new(AliasStruct,TEXT("Command"),RF_Public)UStrProperty ( EC_CppProperty, static_cast<INT>(offsetof(FAlias,Command)), TEXT(""), CPF_Config );
	AliasStruct->Link( ArDummy, 0 );

	// Add alias list to class.
	UStructProperty* Q = new(StaticClass(),TEXT("Aliases"),RF_Public)UStructProperty( CPP_PROPERTY(Aliases), TEXT("Aliases"), CPF_Config, AliasStruct );
	Q->ArrayDim = ALIAS_MAX;

	// Add key list.
	UEnum* InputKeys = FindObjectChecked<UEnum>( AActor::StaticClass(), TEXT("EInputKey") );
	for( INT i=0; i<IK_MAX; i++ )
	{
		if( InputKeys->Names(i)!=NAME_None )
		{
			const TCHAR* Str = *InputKeys->Names(i);
			new(StaticClass(),Str+3,RF_Public)UStrProperty( EC_CppProperty, STRUCT_OFFSET(UInput,Bindings[i]), TEXT("RawKeys"), CPF_Config );
		}
	}
	StaticClass()->Link( ArDummy, 0 );

	// Load defaults.
	StaticClass()->GetDefaultObject()->LoadConfig( 1 );

	unguard;
}

//
// Constructor.
//
UInput::UInput()
{
	guard(UInput::UInput);
	InputKeys = FindObjectChecked<UEnum>( AActor::StaticClass(), TEXT("EInputKey") );
	unguard;
}

//
// Class initializer.
//
void UInput::StaticConstructor()
{
	guard(UInput::StaticConstructor);
	unguard;
}

//
// Serialize
//
void UInput::Serialize( FArchive& Ar )
{
	guard(UInput::Serialize);
	Super::Serialize( Ar );
	Ar << InputKeys;
	unguard;
}

//
// Find a button.
//
BYTE* UInput::FindButtonName( AActor* Actor, const TCHAR* ButtonName ) const
{
	guard(UInput::FindButtonName);
	return hp2_rs_engine_input_find_button_name( &GHp2InputServices, const_cast<UInput*>(this), Actor, reinterpret_cast<const uint32_t*>(ButtonName) );
	unguard;
}

//
// Find an axis.
//
FLOAT* UInput::FindAxisName( AActor* Actor, const TCHAR* ButtonName ) const
{
	guard(UInput::FindAxisName);
	return hp2_rs_engine_input_find_axis_name( &GHp2InputServices, const_cast<UInput*>(this), Actor, reinterpret_cast<const uint32_t*>(ButtonName) );
	unguard;
}

//
// Execute input commands.
//
void UInput::ExecInputCommands( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UInput::ExecInputCommands);
	hp2_rs_engine_input_exec_input_commands( &GHp2InputServices, this, reinterpret_cast<const uint32_t*>(Cmd), &Ar );
	unguard;
}

//
// Init.
//
void UInput::Init( UViewport* InViewport )
{
	guard(UInput::Init);
	hp2_rs_engine_input_init( &GHp2InputServices, this, InViewport );
	unguard;
}

/*-----------------------------------------------------------------------------
	Command line.
-----------------------------------------------------------------------------*/

//
// Execute a command.
//
UBOOL UInput::Exec( const TCHAR* Str, FOutputDevice& Ar )
{
	guard(UInput::Exec);
	return hp2_rs_engine_input_exec( &GHp2InputServices, this, reinterpret_cast<const uint32_t*>(Str), &Ar );
	unguard;
}

/*-----------------------------------------------------------------------------
	Key and axis movement processing.
-----------------------------------------------------------------------------*/

//
// Preprocess input to maintain key tables.
//
UBOOL UInput::PreProcess( EInputKey iKey, EInputAction State, FLOAT Delta )
{
	guard(UInput::PreProcess);
	return hp2_rs_engine_input_pre_process( this, iKey, State );
	unguard;
}

//
// Process input. Returns 1 if handled, 0 if not.
//
UBOOL UInput::Process( FOutputDevice& Ar, EInputKey iKey, EInputAction State, FLOAT Delta )
{
	guard(UInput::Process);
	return hp2_rs_engine_input_process( &GHp2InputServices, this, &Ar, iKey, State, Delta );
	unguard;
}

//
// Direct axis input of variable influence.
//
void UInput::DirectAxis( EInputKey iKey, FLOAT Length, FLOAT Delta )
{
	hp2_rs_engine_input_direct_axis( &GHp2InputServices, this, iKey, Length, Delta );
}

/*-----------------------------------------------------------------------------
	Input reading.
-----------------------------------------------------------------------------*/

//
// Read input for the viewport.
//
void UInput::ReadInput( FLOAT DeltaSeconds, FOutputDevice& Ar )
{
	guard(UInput::ReadInput);

	hp2_rs_engine_input_read_input( &GHp2InputServices, this, DeltaSeconds );

	unguard;
}

/*-----------------------------------------------------------------------------
	Input resetting.
-----------------------------------------------------------------------------*/

//
// Reset the input system's state.
//
void UInput::ResetInput()
{
	guard(UInput::ResetInput);
	hp2_rs_engine_input_reset_input( &GHp2InputServices, this );

	unguard;
}

/*-----------------------------------------------------------------------------
	Actor input natives.
-----------------------------------------------------------------------------*/

void AActor::execGetCurrentKeyState( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execGetCurrentKeyState);

	P_GET_BYTE(Key);
	P_FINISH;

	UBOOL ReturnValue = hp2_rs_engine_input_current_key_state( &GHp2InputServices, this, Key );
	appMemcpy(Result, &ReturnValue, sizeof(ReturnValue));

	unguardSlow;
}

/*-----------------------------------------------------------------------------
	Utility functions.
-----------------------------------------------------------------------------*/

//
// Return the name of a key.
//
const TCHAR* UInput::GetKeyName( EInputKey Key ) const
{
	guard(UInput::GetKeyName);
	return reinterpret_cast<const TCHAR*>( hp2_rs_engine_input_get_key_name( &GHp2InputServices, const_cast<UInput*>(this), Key ) );
	unguard;
}

//
// Find the index of a named key.
//
UBOOL UInput::FindKeyName( const TCHAR* KeyName, EInputKey& iKey ) const
{
	guard(UInput::FindKeyName);
	return hp2_rs_engine_input_find_key_name( &GHp2InputServices, const_cast<UInput*>(this), reinterpret_cast<const uint32_t*>(KeyName), reinterpret_cast<INT*>(&iKey) );
	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
