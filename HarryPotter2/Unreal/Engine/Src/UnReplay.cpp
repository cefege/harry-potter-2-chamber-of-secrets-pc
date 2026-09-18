/*=============================================================================
	FReplay.cpp: Unreal replay.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Scott Peter.
=============================================================================*/

#include "EnginePrivate.h"
#include <stdlib.h>

/*-----------------------------------------------------------------------------
	UReplay class implementation.
-----------------------------------------------------------------------------*/

FReplay::FReplay()
:	FileAr(NULL), bRecording(false), bInReplay(false), bPaused(false)
{
}

FReplay::~FReplay()
{
	delete FileAr;
	FileAr = NULL;
}

bool FReplay::OpenFile( const TCHAR* InName, bool bRecord, const TCHAR* StartURL, FOutputDevice& Ar )
{
	if( Recording() )
	{
		Ar.Logf( TEXT("Already recording: %s"), *FileName );
		return false;
	}
	else if( Replaying() )
	{
		Ar.Logf( TEXT("Already replaying: %s"), *FileName );
		return false;
	}

	FileName = FString(appUserDir()) + InName + TEXT(".rep");

	if( bRecord )
		FileAr = GFileManager->CreateFileWriter( *FileName );
	else
		FileAr = GFileManager->CreateFileReader( *FileName );
	if( !FileAr )
	{
		Ar.Logf( TEXT("Cannot %s file %s"), bRecord ? TEXT("create") : TEXT("open"), *FileName );
		return false;
	}

	bRecording = bRecord;
	bInReplay = false;
	FrameNum = 0;

	// Copy/serialise starting URL.
	URLStr = StartURL;
	*FileAr << URLStr;

	if( !bRecording )
	{
		// Read first tick value.
		FInputEvent IE;
		SerializeFrameInput( IE );
	}

	// Re-seed random number generator for all record/replays.
	srand(1);
	appResetRandTraceForReplay();

	return true;
}

bool FReplay::Stop()
{
	delete FileAr;
	FileAr = NULL;
	bPaused = false;
	return true;
}

bool FReplay::SerializeFrameTick( float& TickDelta )
{
	if( Recording() )
	{
		// Save tick as a special input event.
		FInputEvent IE = { IK_Play, IST_Axis, TickDelta };
		*FileAr << IE;
		FileAr->Flush();
	}
	else if( Replaying() )
	{
		// Get previously read tick.
		TickDelta = NextTick;
		if( PauseCount > 0 )
			PauseCount--;
	}

	FrameNum++;
	return true;
}

bool FReplay::SerializeFrameInput( FInputEvent& IE )
{
	if( FileAr )
	{
		if( Replaying() && FileAr->AtEnd() )
		{
			// Finished.
			delete FileAr;
			FileAr = NULL;
			return bInReplay = false;
		}

		*FileAr << IE;
		if( Replaying() && IE.iKey == IK_Play && IE.State == IST_Axis )
		{
			// Done with frame.
			NextTick = IE.Delta;
			return bInReplay = false;
		}
		return bInReplay = true;
	}
	else
		return false;
}

