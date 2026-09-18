/*=============================================================================
	FOutputDeviceSDLError.h: SDL error output device.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Stijn Volckaert
=============================================================================*/

#include "Core.h"
#include <SDL2/SDL.h>
#include <cstring>

//
// SDL output device.
//
class FOutputDeviceSDLError : public FOutputDeviceError
{
	INT ErrorPos;
	EName ErrorType;
	void LocalPrint( const TCHAR* Str )
	{
		wprintf(TEXT("%ls"),Str);
	}
public:
	FOutputDeviceSDLError()
	: ErrorPos(0)
	, ErrorType(NAME_None)
	{}
	void Serialize( const TCHAR* Msg, enum EName Event )
	{
#if defined(_DEBUG) && 0
		// Just display info and break the debugger.
  		debugf( NAME_Critical, TEXT("appError called while debugging:") );
		debugf( NAME_Critical, Msg );
		UObject::StaticShutdownAfterError();
  		debugf( NAME_Critical, TEXT("Breaking debugger") );
		// stijn: the pointer needs to be volatile, otherwise the compiler can
		// remove the null deref even at low optimization levels.
		*(volatile BYTE*)NULL=0;
#else
		if( !GIsCriticalError )
		{
			// First appError.
			GIsCriticalError = 1;
			ErrorType        = Event;
			debugf( NAME_Critical, TEXT("appError called:") );
			debugf( NAME_Critical, Msg );

			// Shut down.
			UObject::StaticShutdownAfterError();
			appStrncpy( GErrorHist, Msg, ARRAY_COUNT(GErrorHist) );
			appStrncat( GErrorHist, TEXT("\r\n\r\n"), ARRAY_COUNT(GErrorHist) );
			ErrorPos = appStrlen(GErrorHist);
			if( GIsGuarded )
			{
				appStrncat( GErrorHist, LocalizeError("History",TEXT("Core")), ARRAY_COUNT(GErrorHist) );
				appStrncat( GErrorHist, TEXT(": "), ARRAY_COUNT(GErrorHist) );
			}
			else
			{
				HandleError();
			}
		}
		else debugf( NAME_Critical, TEXT("Error reentered: %s"), Msg );

		// Propagate the error or exit.
		if( GIsGuarded )
			throw( 1 );
		else
			appRequestExit( 1 );
#endif
	}
	void HandleError()
	{
		try
		{
			GIsGuarded       = 0;
			GIsRunning       = 0;
			GIsCriticalError = 1;
			GLogHook         = NULL;
			UObject::StaticShutdownAfterError();

			debugf(TEXT("HandleError"));

			GErrorHist[ErrorType==NAME_FriendlyError ? ErrorPos : ARRAY_COUNT(GErrorHist)-1]=0;
			LocalPrint(GErrorHist);
			ANSICHAR Title[256];
			ANSICHAR Message[ARRAY_COUNT(GErrorHist) * 4];
			if( !appToUtf8InPlace(Title, LocalizeError("Warning",TEXT("Core")), ARRAY_COUNT(Title)) )
			{
				std::strncpy(Title, "Error", sizeof(Title));
				Title[sizeof(Title)-1] = 0;
			}
			if( !appToUtf8InPlace(Message, GErrorHist, ARRAY_COUNT(Message)) )
			{
				std::strncpy(Message, "Unreal runtime error", sizeof(Message));
				Message[sizeof(Message)-1] = 0;
			}
			if( !ParseParam(appCmdLine(), TEXT("NOFRONTEND")) )
				SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, Title, Message, NULL);

			LocalPrint( TEXT("\n\nExiting due to error\n") );
		}
		catch( ... )
		{}
	}
};
