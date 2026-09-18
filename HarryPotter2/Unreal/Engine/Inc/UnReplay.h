/*=============================================================================
	UnReplay.h: Unreal replay recording.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Scott Peter.
=============================================================================*/

/*-----------------------------------------------------------------------------
	UReplay.
-----------------------------------------------------------------------------*/

//
// Record and reproduce a session as exactly as possible, by recreating player input
// and timer data. Differs from UDemoDriver, which does not actually run the game,
// but records and plays back net replication data.
//
class ENGINE_API FReplay
{
public:
	// Constructors.
	FReplay();
	~FReplay();

	// Status functions.
	bool Recording() const
	{
		return FileAr && bRecording;
	}
	bool Replaying() const
	{
		return FileAr && !bRecording;
	}
	bool InReplay() const
	{
		// Flag that input is currently coming from replay system.
		// Set during processing of replay events.
		return bInReplay;
	}
	const FString& GetFileName() const
	{
		return FileName;
	}
	const FString& GetURLStr() const
	{
		// Starting level URL this was recorded with.
		return URLStr;
	}
	int GetFrameNum() const
	{
		return FrameNum;
	}
	void Pause( bool bPause, int Count = 0 )
	{
		bPaused = bPause;
		PauseCount = Count;
	}
	bool Paused() const
	{
		return bPaused && PauseCount == 0;
	}

	// File functions.
	bool OpenFile( const TCHAR* InName, bool bRecord, const TCHAR* StartURL, FOutputDevice& Ar );
	bool Record( const TCHAR* InName, const TCHAR* StartURL, FOutputDevice& Ar )
	{
		return OpenFile( InName, true, StartURL, Ar );
	}
	bool Replay( const TCHAR* InName, FOutputDevice& Ar )
	{
		return OpenFile( InName, false, NULL, Ar );
	}
	bool Stop();

	// Data functions.
	bool SerializeFrameTick( float& Tick );

	struct FInputEvent
	{
		EInputKey iKey;
		EInputAction State;
		FLOAT Delta;

		friend FArchive& operator<<( FArchive& Ar, FInputEvent& IE );
	};

	bool SerializeFrameInput( FInputEvent& IE );

	// Write-only version.
	bool SerializeFrameInput( EInputKey iKey, EInputAction State, float Delta = 0.f )
	{
		FInputEvent IE = { iKey, State, Delta };
		return SerializeFrameInput( IE );
	}

protected:
	// Variables.
	FString			FileName;
	FArchive*		FileAr;
	FString			URLStr;
	bool			bRecording;
	bool			bInReplay;
	bool			bPaused;
	int				PauseCount;			// Frames to run until pausing.
	int				FrameNum;
	float			NextTick;
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
