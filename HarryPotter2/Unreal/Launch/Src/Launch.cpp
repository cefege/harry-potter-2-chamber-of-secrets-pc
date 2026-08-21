/*=============================================================================
	Launch.cpp: Game launcher.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney.
=============================================================================*/

#include "LaunchPrivate.h"
#include "UnEngineWin.h"

/*-----------------------------------------------------------------------------
	Global variables.
-----------------------------------------------------------------------------*/

// General.
extern "C" {HINSTANCE hInstance;}
extern "C" {TCHAR GPackage[64]=TEXT("Launch");}

// Memory allocator.

#include "FMallocWindows.h"
#include "FMallocDebug.h"
#ifdef _DEBUG
	TMallocDebug<FMallocWindows> Malloc;
#else
	FMallocWindows Malloc;
#endif

TMallocLog<FMallocWindows> LogMalloc;

// Log file.
#include "FOutputDeviceFile.h"
FOutputDeviceFile Log;

// Error handler.
#include "FOutputDeviceWindowsError.h"
FOutputDeviceWindowsError Error;

// Feedback.
#include "FFeedbackContextWindows.h"
FFeedbackContextWindows Warn;

// File manager.
#include "FFileManagerWindows.h"
FFileManagerWindows FileManager;

// Config.
#include "FConfigCacheIni.h"

/*-----------------------------------------------------------------------------
	SafeDisk stuff.
-----------------------------------------------------------------------------*/
#include "CdaPfn.h"

int	CheckSafeDiskProtection( void );

CDAPFN_DECLARE_GLOBAL(CheckSafeDiskProtection, CDAPFN_OVERHEAD_L5, CDAPFN_CONSTRAINT_NONE);

int	CheckSafeDiskProtection( void )
{
	CDAPFN_ENDMARK(CheckSafeDiskProtection);

	return 0;
}

/*-----------------------------------------------------------------------------
	WinMain.
-----------------------------------------------------------------------------*/


//
// Main entry point.
// This is an example of how to initialize and launch the engine.
//
INT WINAPI WinMain( HINSTANCE hInInstance, HINSTANCE hPrevInstance, char*, INT nCmdShow )
{
	// Remember instance.
	INT ErrorLevel = 0;
	GIsStarted     = 1;
	hInstance      = hInInstance;
	const TCHAR* CmdLine = GetCommandLine();
	appStrcpy( GPackage, appPackage() );

	//	Check our copy protection sceme - bail if it fails.  -tg
	//
	try
	{
		CheckSafeDiskProtection();
	}
	catch( ... )
	{
		MessageBox( NULL, LocalizeError(TEXT("Copy protection failed.")), LocalizeError(TEXT("SafeDisk Error")), MB_OK );
		return 0;
	}

	// See if this should be passed to another instances.
	if
	(	!appStrfind(CmdLine,TEXT("Server"))
	&&	!appStrfind(CmdLine,TEXT("NewWindow"))
	&&	!appStrfind(CmdLine,TEXT("changevideo"))
	&&	!appStrfind(CmdLine,TEXT("TestRenDev")) )
	{
		TCHAR ClassName[256];
		MakeWindowClassName(ClassName,TEXT("WLog"));
		for( HWND hWnd=NULL; ; )
		{
			hWnd = TCHAR_CALL_OS(FindWindowExW(hWnd,NULL,ClassName,NULL),FindWindowExA(hWnd,NULL,TCHAR_TO_ANSI(ClassName),NULL));
			if( !hWnd )
				break;
			if( GetPropX(hWnd,TEXT("IsBrowser")) )
			{
				while( *CmdLine && *CmdLine!=' ' )
					CmdLine++;
				if( *CmdLine==' ' )
					CmdLine++;
				COPYDATASTRUCT CD;
				DWORD Result;
				CD.dwData = WindowMessageOpen;
				CD.cbData = (appStrlen(CmdLine)+1)*sizeof(TCHAR*);
				CD.lpData = const_cast<TCHAR*>( CmdLine );
				SendMessageTimeout( hWnd, WM_COPYDATA, (WPARAM)NULL, (LPARAM)&CD, SMTO_ABORTIFHUNG|SMTO_BLOCK, 30000, &Result );
				GIsStarted = 0;
				return 0;
			}
		}
	}

	// Begin guarded code.
#ifndef _DEBUG
	try
	{
#endif
		UEngine* Engine = NULL;

		// Init core.
		GIsClient = GIsGuarded = 1;
		FMalloc* PMalloc = &Malloc;
		if( ParseParam(CmdLine, TEXT("MEMSTAT")) )
			PMalloc = &LogMalloc;
		appInit( GPackage, CmdLine, PMalloc, &Log, &Error, &Warn, &FileManager, FConfigCacheIni::Factory, 1 );

		// Init mode.
		GIsServer     = 1;
		GIsClient     = !ParseParam(appCmdLine(),TEXT("SERVER"));
		GIsEditor     = 0;
		GIsScriptable = 1;
		GLazyLoad     = !GIsClient || ParseParam(appCmdLine(),TEXT("LAZY"));

		// Figure out whether to show log or splash screen.

		// --- Splash Screens
		// Figure out whether to show log or splash screen.
		UBOOL ShowLog = ParseParam(appCmdLine(),TEXT("LOG"));
		INT RunCount  = appAtoi(GConfig->GetStr(TEXT("Engine.Engine"),TEXT("RunCount")));
		FString lang  = GConfig->GetStr(TEXT("Engine.Engine"),TEXT("Language"));
		FString Filename = FString::Printf(TEXT("..\\Help\\EALogo%s.bmp"),lang);
		GConfig->SetString(TEXT("Engine.Engine"),TEXT("RunCount"),*FString::Printf(TEXT("%i"),RunCount+1));
		if( GFileManager->FileSize(*Filename)<0 )
			Filename = FString(TEXT("..\\Help")) * TEXT("EALogo1.bmp");
		if( GFileManager->FileSize(*Filename)<0 )
			Filename = TEXT("..\\Help\\EALogo1.bmp");
		appStrcpy( GPackage, appPackage() );
		
		// If we came from the FrontEnd then don't show the EA and WB splash screens again
		if( !ParseParam(appCmdLine(),TEXT("SAVESLOT=") ) && !ParseParam(appCmdLine(),TEXT("NOFRONTEND")) )
		{
			if( !ShowLog && !ParseParam(appCmdLine(),TEXT("server")) && !appStrfind(appCmdLine(),TEXT("TestRenDev")) )
				InitSplash( *Filename );
			
			FString WaitTime;
			
			// Wait for 2.0 seconds then exit this splash screen
			if( ThreadId )
			{
				WaitTime = GConfig->GetStr(TEXT("Engine.Engine"),TEXT("EASplashWaitTime"));
				
				if(WaitTime.Len())
					appSleep(appAtof(*WaitTime));
				ExitSplash();
			}
			
			// Init our second splash screen and wait
			Filename = FString::Printf(TEXT("..\\Help\\WBLegal%s.bmp"),lang);
			if( GFileManager->FileSize(*Filename)<0 )
				Filename = FString(TEXT("..\\Help")) * TEXT("WBLegal.bmp");
			if( GFileManager->FileSize(*Filename)<0 )
				Filename = TEXT("..\\Help\\WBLegal.bmp");
			if( !ShowLog && !ParseParam(appCmdLine(),TEXT("server")) && !appStrfind(appCmdLine(),TEXT("TestRenDev")) )
				InitSplash( *Filename );
			
			// Wait for 2.0 seconds then exit this splash screen
			if( ThreadId )
			{
				WaitTime = GConfig->GetStr(TEXT("Engine.Engine"),TEXT("WBSplashWaitTime"));
				if(WaitTime.Len())
					appSleep(appAtof(*WaitTime));
				ExitSplash();
			}
		}
		// Init windowing.
		InitWindowing();
		
		// Create log window, but only show it if ShowLog.
		GLogWindow = new WLog( Log.Filename, Log.LogAr, TEXT("GameLog") );
		GLogWindow->OpenWindow( ShowLog, 0 );
		GLogWindow->Log( NAME_Title, LocalizeGeneral("Start") );
		if( GIsClient )
			SetPropX( *GLogWindow, TEXT("IsBrowser"), (HANDLE)1 );
		
		// Check that only the demo is being launched
		FString DemoFileName( FString(TEXT("Engine.dll")) );
		INT EngineFileSize = GFileManager->FileSize( *DemoFileName );
		debugf( TEXT("Looking for file: %s %d"), *DemoFileName, EngineFileSize );
		
		DemoFileName = FString(TEXT("HPMenu.u"));
		INT HPMenuFileSize = GFileManager->FileSize( *DemoFileName );
		debugf( TEXT("Looking for file: %s %d"), *DemoFileName, HPMenuFileSize );
		
		DemoFileName = FString(TEXT("HarryPotter.u"));
		INT HarryPFileSize = GFileManager->FileSize( *DemoFileName );
		debugf( TEXT("Looking for file: %s %d"), *DemoFileName, HarryPFileSize );

//		if( ( EngineFileSize != 2121728 ) ||
//			( HPMenuFileSize >  7286000 ) ||	// 5% headroom for future script recompiles...
//			( HarryPFileSize >  4896000 ) )
//		{
//			// Show error message
//			FString Error = TEXT("Sorry, this demo program can not be used to run the retail version of Harry Potter.");
//			FString Title = TEXT("Failed Launching Program");
//			::MessageBox( NULL, *Error, *Title, MB_OK|MB_TASKMODAL|MB_SETFOREGROUND );
//		} 
//		else
		{
			// Init engine.
			Engine = InitEngine();
			if( Engine )
			{
				GLogWindow->Log( NAME_Title, LocalizeGeneral("Run") );

				// Hide splash screen.
				ExitSplash();

				// Optionally Exec an exec file
				FString Temp;
				if( Parse(CmdLine, TEXT("EXEC="), Temp) )
				{
					Temp = FString(TEXT("exec ")) + Temp;
					if( Engine->Client && Engine->Client->Viewports.Num() && Engine->Client->Viewports(0) )
						Engine->Client->Viewports(0)->Exec( *Temp, *GLogWindow );
				}

				// Start main engine loop, including the Windows message pump.
				if( !GIsRequestingExit )
					MainLoop( Engine );
			}
		}

		// Clean shutdown.
		if( !GIsCriticalError )
			GMalloc->DumpAllocs();
		GFileManager->Delete( *(FString(appUserDir()) * TEXT("Running.ini")), 0,0 );
		RemovePropX( *GLogWindow, TEXT("IsBrowser") );
		GLogWindow->Log( NAME_Title, LocalizeGeneral("Exit") );
		delete GLogWindow;

		if( Engine && Engine->Audio )
			Engine->Audio->SetViewport( NULL ); //kill the audio cleanly

		appPreExit();
		GIsGuarded = 0;
#ifndef _DEBUG
	}
	catch( ... )
	{
		// Crashed.
		ErrorLevel = 1;
		Error.HandleError();
	}
#endif

	// Final shut down.
	appExit();
	GIsStarted = 0;
	return ErrorLevel;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
