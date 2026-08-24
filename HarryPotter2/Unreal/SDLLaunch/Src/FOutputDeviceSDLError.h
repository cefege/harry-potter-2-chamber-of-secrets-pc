/*=============================================================================
	FOutputDeviceSDLError.h: SDL error output device.
=============================================================================*/

#ifndef _FOUTPUTDEVICESDLERROR_H
#define _FOUTPUTDEVICESDLERROR_H

#include <SDL2/SDL.h>

#include <string>
#include <unistd.h>

#include "Core.h"

//
// Fatal-error device. Mirrors the core ANSI error flow while surfacing the
// error history as a UTF-8 message box on top of the plain-text stderr dump.
// Headless runs (-NOFRONTEND) never open a window so scripted sessions stay
// unattended; the text output always happens.
//
class FOutputDeviceSDLError : public FOutputDeviceError
{
private:
	INT ErrorPos;
	EName ErrorType;

	// Converts engine text to UTF-8 for SDL's narrow-char interfaces.
	static std::string ToUtf8( const TCHAR* Text )
	{
		ANSICHAR Buffer[8192];
		Buffer[0] = 0;
		if( !Text || !appToUtf8InPlace( Buffer, Text, ARRAY_COUNT(Buffer) ) )
			return std::string();
		return std::string( Buffer );
	}

	void LocalPrint( const TCHAR* Str )
	{
#if UNICODE
		wprintf( TEXT("%ls"), Str );
#else
		printf( TEXT("%s"), Str );
#endif
	}

	void ShowErrorBox( const TCHAR* Message )
	{
		if( ParseParam( appCmdLine(), TEXT("NOFRONTEND") ) )
			return;
		// The modal box needs a human to dismiss it; unattended sessions
		// (harness, CI) would hang here forever. Skip it whenever stdin is
		// not a terminal so fatal errors exit deterministically.
		if( !isatty( STDIN_FILENO ) )
			return;
		const std::string Utf8 = ToUtf8( Message );
		SDL_ShowSimpleMessageBox(
			SDL_MESSAGEBOX_ERROR,
			"Runtime Error",
			Utf8.c_str(),
			NULL );
	}

public:
	FOutputDeviceSDLError()
	: ErrorPos( 0 )
	, ErrorType( NAME_None )
	{}

	void Serialize( const TCHAR* Msg, enum EName Event )
	{
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
			if( !GIsGuarded )
				HandleError();
		}
		else debugf( NAME_Critical, TEXT("Error reentered: %s"), Msg );

		// Propagate the error or exit.
		if( GIsGuarded )
			throw( 1 );
		else
			appRequestExit( 1 );
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
			GErrorHist[ErrorType==NAME_FriendlyError ? ErrorPos : ARRAY_COUNT(GErrorHist)-1] = 0;
			LocalPrint( GErrorHist );
			LocalPrint( TEXT("\n\nExiting due to error\n") );
			fflush( stdout );
			ShowErrorBox( GErrorHist );
		}
		catch( ... )
		{}
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

#endif
