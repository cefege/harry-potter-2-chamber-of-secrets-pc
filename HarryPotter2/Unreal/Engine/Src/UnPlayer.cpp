/*=============================================================================
	UnActor.cpp: Actor list functions.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"
#include "Hp2PlayerServices.h"

/*-----------------------------------------------------------------------------
	UPlayer object implementation.
-----------------------------------------------------------------------------*/

void UPlayer::StaticConstructor()
{
	guard(UPlayer::StaticConstructor);
	new(GetClass(),TEXT("Actor"),RF_Public)UObjectProperty(CPP_PROPERTY(Actor),TEXT("Player"),CPF_Transient,APlayerPawn::StaticClass());
	unguard;
}

UPlayer::UPlayer()
{}
void UPlayer::Serialize( FArchive& Ar )
{
	guard(UPlayer::Serialize);
	Super::Serialize( Ar );
	unguard;
}
void UPlayer::Destroy()
{
	guard(UPlayer::Destroy);
	hp2_rs_engine_player_destroy( &GHp2PlayerServices, this );
	Super::Destroy();
	unguard;
}
UBOOL UPlayer::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UPlayer::Exec);
	return hp2_rs_engine_player_exec( &GHp2PlayerServices, this, Cmd, &Ar );
	unguard;
}
IMPLEMENT_CLASS(UPlayer);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
