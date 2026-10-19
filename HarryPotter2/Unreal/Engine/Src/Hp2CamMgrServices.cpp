#include "EnginePrivate.h"
#include "UnRender.h"
#include "Hp2CamMgrServices.h"

// Thunks for the Rust UnCamMgr port. Each one is a single original C++ call.

static int Hp2CamMgrParseBrightnessCommand( const void* Cmd )
{
	const TCHAR* Stream = static_cast<const TCHAR*>(Cmd);
	return ParseCommand( &Stream, TEXT("BRIGHTNESS") );
}

static void Hp2CamMgrClientEngineFlush( void* Client, int AllowPrecache )
{
	static_cast<UClient*>(Client)->Engine->Flush( AllowPrecache );
}

static void Hp2CamMgrClientSaveConfig( void* Client )
{
	static_cast<UClient*>(Client)->SaveConfig();
}

static int Hp2CamMgrClientViewportsNum( void* Client )
{
	return static_cast<UClient*>(Client)->Viewports.Num();
}

static void* Hp2CamMgrClientViewport0Actor( void* Client )
{
	return static_cast<UClient*>(Client)->Viewports(0)->Actor;
}

static void Hp2CamMgrClientBrightnessMessage( void* Client, int Level )
{
	static_cast<UClient*>(Client)->Viewports(0)->Actor->eventClientMessage(
		*FString::Printf( TEXT("Brightness level %i/10"), Level ), NAME_None, 0 );
}

static void* Hp2CamMgrClientViewportRenderDev( void* Client, int Index )
{
	return static_cast<UClient*>(Client)->Viewports(Index)->RenDev;
}

static void Hp2CamMgrRenderDevFlush( void* RenDev, int AllowPrecache )
{
	static_cast<URenderDevice*>(RenDev)->Flush( AllowPrecache );
}

const Hp2CamMgrServices GHp2CamMgrServices =
{
	Hp2CamMgrParseBrightnessCommand,
	Hp2CamMgrClientEngineFlush,
	Hp2CamMgrClientSaveConfig,
	Hp2CamMgrClientViewportsNum,
	Hp2CamMgrClientViewport0Actor,
	Hp2CamMgrClientBrightnessMessage,
	Hp2CamMgrClientViewportRenderDev,
	Hp2CamMgrRenderDevFlush,
};
