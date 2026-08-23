/*=============================================================================
	FFeedbackContextSDL.h: SDL feedback context.
=============================================================================*/

#ifndef _FFEEDBACKCONTEXTSDL_H
#define _FFEEDBACKCONTEXTSDL_H

#include <SDL2/SDL.h>

#include <string>

#include "Core.h"

//
// User-interaction feedback context. Text output mirrors the core ANSI
// context; errors additionally surface as UTF-8 message boxes and Yes/No
// prompts become native dialog boxes. Runs launched with -NOFRONTEND (or
// -Silent) never open a window: they answer prompts with the safe default
// and keep everything on stdout.
//
class FFeedbackContextSDL : public FFeedbackContext
{
public:
	// Variables.
	INT SlowTaskCount;
	INT WarningCount;
	FContextSupplier* Context;
	FOutputDevice* AuxOut;

	// Constructor.
	FFeedbackContextSDL()
	: SlowTaskCount( 0 )
	, WarningCount( 0 )
	, Context( NULL )
	, AuxOut( NULL )
	{}

private:
	void LocalPrint( const TCHAR* Str )
	{
#if UNICODE
		wprintf(TEXT("%ls"),Str);
#else
		printf(TEXT("%s"),Str);
#endif
	}

	static UBOOL FrontendSuppressed()
	{
		return ParseParam( appCmdLine(), TEXT("NOFRONTEND") )
			|| ParseParam( appCmdLine(), TEXT("Silent") );
	}

	static std::string ToUtf8( const TCHAR* Text )
	{
		ANSICHAR Buffer[8192];
		Buffer[0] = 0;
		if( !Text || !appToUtf8InPlace( Buffer, Text, ARRAY_COUNT(Buffer) ) )
			return std::string();
		return std::string( Buffer );
	}

	static void ShowErrorBox( const TCHAR* Message )
	{
		const std::string Utf8 = ToUtf8( Message );
		SDL_ShowSimpleMessageBox(
			SDL_MESSAGEBOX_ERROR,
			"Game Feedback",
			Utf8.c_str(),
			NULL );
	}

public:
	void Serialize( const TCHAR* V, enum EName Event )
	{
		TCHAR Temp[1024]=TEXT("");
		if( Event==NAME_Title )
		{
			return;
		}
		else if( Event==NAME_Heading )
		{
			appSprintf( Temp, TEXT("--------------------%s--------------------"), (TCHAR*)V );
			V = Temp;
		}
		else if( Event==NAME_SubHeading )
		{
			appSprintf( Temp, TEXT("%s..."), (TCHAR*)V );
			V = Temp;
		}
		else if( Event==NAME_Error || Event==NAME_Warning || Event==NAME_ExecWarning || Event==NAME_ScriptWarning )
		{
			if( Context )
			{
				appSprintf( Temp, TEXT("%s : %s, %s"), *Context->GetContext(), *FName(Event), (TCHAR*)V );
				V = Temp;
			}
			WarningCount++;
		}
		else if( Event==NAME_Progress )
		{
			LocalPrint( V );
			LocalPrint( TEXT("\r") );
			fflush( stdout );
			return;
		}

		LocalPrint( V );
		LocalPrint( TEXT("\n") );
		fflush( stdout );
		if( GLog != this )
			GLog->Serialize( V, Event );
		if( AuxOut )
			AuxOut->Serialize( V, Event );

		if( Event==NAME_Error && !FrontendSuppressed() )
			ShowErrorBox( V );
	}

	UBOOL YesNof( const TCHAR* Fmt, ... )
	{
		TCHAR TempStr[4096];
		GET_VARARGS( TempStr, ARRAY_COUNT(TempStr), Fmt );

		if( FrontendSuppressed() || !(GIsClient || GIsEditor) )
		{
			LocalPrint( TempStr );
			LocalPrint( TEXT("\n") );
			fflush( stdout );
			return 1;
		}

		const SDL_MessageBoxButtonData Buttons[] =
		{
			{ SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT|SDL_MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT, 1, "Yes" },
			{ 0,                                                                                0, "No"  }
		};
		const SDL_MessageBoxData Box =
		{
			SDL_MESSAGEBOX_INFORMATION,
			NULL,
			"Confirmation",
			ToUtf8( TempStr ).c_str(),
			(int)ARRAY_COUNT(Buttons),
			Buttons,
			NULL
		};
		int Pressed = 0;
		if( SDL_ShowMessageBox( &Box, &Pressed ) < 0 )
			return 1;
		return Pressed != 0;
	}

	void BeginSlowTask( const TCHAR* Task, UBOOL StatusWindow, UBOOL Cancelable )
	{
		GIsSlowTask = ++SlowTaskCount>0;
	}

	void EndSlowTask()
	{
		check(SlowTaskCount>0);
		GIsSlowTask = --SlowTaskCount>0;
	}

	UBOOL VARARGS StatusUpdatef( INT Numerator, INT Denominator, const TCHAR* Fmt, ... )
	{
		TCHAR TempStr[4096];
		GET_VARARGS( TempStr, ARRAY_COUNT(TempStr), Fmt );
		if( GIsSlowTask )
		{
			LocalPrint( TempStr );
			LocalPrint( TEXT("\r") );
			fflush( stdout );
		}
		return 1;
	}

	void SetContext( FContextSupplier* InSupplier )
	{
		Context = InSupplier;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

#endif
