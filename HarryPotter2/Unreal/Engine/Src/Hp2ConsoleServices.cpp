/*=============================================================================
	Hp2ConsoleServices.cpp: Console services called by the Rust engine port
	(crates/hp2-engine/src/console.rs). Each adapter decodes raw pointers into
	the C++ types and forwards to the engine call the original UnCon.cpp body
	made.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "Hp2ConsoleServices.h"

static void ConsoleInitExecution( void* Console )
{
	((UConsole*)Console)->InitExecution();
}

static const void* ConsoleLocalizeGeneral( const char* Key, const void* Package )
{
	return LocalizeGeneral( Key, (const TCHAR*)Package );
}

static void ConsoleLogFormat( void* Console, const void* Format )
{
	((UConsole*)Console)->Logf( (const TCHAR*)Format );
}

static void ConsoleEventMessage( void* Console, const void* Data, INT Name )
{
	((UConsole*)Console)->eventMessage( 0, (const TCHAR*)Data, 0, (EName)Name );
}

static void ConsoleEventTick( void* Console, FLOAT DeltaTime )
{
	((UConsole*)Console)->eventTick( DeltaTime );
}

static INT ConsoleViewportExec( void* Viewport, const void* Cmd, void* Console )
{
	return ((UViewport*)Viewport)->Exec( (const TCHAR*)Cmd, *(UConsole*)Console );
}

static void* ConsoleViewportCreateNativeFont( void* Viewport, const void* Name, INT Height )
{
	return ((UViewport*)Viewport)->CreateNativeFont( (const TCHAR*)Name, Height );
}

static FLOAT ConsoleViewportTimeDelta( void* Viewport )
{
	return ((UViewport*)Viewport)->CurrentTime - ((UViewport*)Viewport)->LastUpdateTime;
}

static void ConsoleSaveStringToFile( const void* String, const void* Filename )
{
	appSaveStringToFile( *(const FString*)String, (const TCHAR*)Filename, GFileManager );
}

static void ConsoleFrameComputeRenderSize( void* Frame )
{
	((FSceneNode*)Frame)->ComputeRenderSize();
}

const Hp2ConsoleServices GHp2ConsoleServices =
{
	ConsoleInitExecution,
	ConsoleLocalizeGeneral,
	ConsoleLogFormat,
	ConsoleEventMessage,
	ConsoleEventTick,
	ConsoleViewportExec,
	ConsoleViewportCreateNativeFont,
	ConsoleViewportTimeDelta,
	ConsoleSaveStringToFile,
	ConsoleFrameComputeRenderSize,
};
