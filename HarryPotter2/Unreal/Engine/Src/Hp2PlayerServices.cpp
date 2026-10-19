// C callbacks behind the Rust UnPlayer port (crates/hp2-engine/src/player.rs).
// Each thunk performs one member read or virtual call from the original UPlayer code,
// so the Rust side never re-implements engine object layout or dispatch.
// Object pointers arrive as void*. Every engine class here derives from UObject by
// single inheritance, so the UObject* conversions below are identity casts.

#include "EnginePrivate.h"
#include "Hp2PlayerServices.h"

static int Hp2PlayerIsRunning( void )
{
	return GIsRunning;
}

static void* Hp2PlayerPlayerActor( void* Player )
{
	return static_cast<UPlayer*>(Player)->Actor;
}

static void Hp2PlayerSetPlayerActor( void* Player, void* Actor )
{
	static_cast<UPlayer*>(Player)->Actor = static_cast<APlayerPawn*>(Actor);
}

static void* Hp2PlayerActorGetLevel( void* Actor )
{
	return static_cast<AActor*>(Actor)->GetLevel();
}

static void Hp2PlayerActorClearPlayer( void* Actor )
{
	static_cast<APlayerPawn*>(Actor)->Player = NULL;
}

static void Hp2PlayerLevelDestroyActor( void* Level, void* Actor, int Force )
{
	static_cast<ULevel*>(Level)->DestroyActor( static_cast<AActor*>(Actor), Force );
}

static int Hp2PlayerLevelExec( void* Level, const void* Cmd, void* Ar )
{
	return static_cast<ULevel*>(Level)->Exec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar) );
}

static void* Hp2PlayerActorLevel( void* Actor )
{
	return static_cast<AActor*>(Actor)->Level;
}

static void* Hp2PlayerLevelInfoGame( void* LevelInfo )
{
	return static_cast<ALevelInfo*>(LevelInfo)->Game;
}

static int Hp2PlayerObjectScriptConsoleExec( void* Obj, const void* Cmd, void* Ar, void* Executor )
{
	return static_cast<UObject*>(Obj)->ScriptConsoleExec(
		static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar), static_cast<UObject*>(Executor) );
}

static void* Hp2PlayerActorHud( void* Actor )
{
	return static_cast<APlayerPawn*>(Actor)->myHUD;
}

static void* Hp2PlayerActorInventory( void* Actor )
{
	return static_cast<AActor*>(Actor)->Inventory;
}

static void* Hp2PlayerInventoryNext( void* Inv )
{
	return static_cast<AInventory*>(Inv)->Inventory;
}

static void* Hp2PlayerLevelEngine( void* Level )
{
	return static_cast<ULevel*>(Level)->Engine;
}

static int Hp2PlayerEngineExec( void* Engine, const void* Cmd, void* Ar )
{
	return static_cast<UEngine*>(Engine)->Exec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar) );
}

const Hp2PlayerServices GHp2PlayerServices =
{
	Hp2PlayerIsRunning,
	Hp2PlayerPlayerActor,
	Hp2PlayerSetPlayerActor,
	Hp2PlayerActorGetLevel,
	Hp2PlayerActorClearPlayer,
	Hp2PlayerLevelDestroyActor,
	Hp2PlayerLevelExec,
	Hp2PlayerActorLevel,
	Hp2PlayerLevelInfoGame,
	Hp2PlayerObjectScriptConsoleExec,
	Hp2PlayerActorHud,
	Hp2PlayerActorInventory,
	Hp2PlayerInventoryNext,
	Hp2PlayerLevelEngine,
	Hp2PlayerEngineExec,
};
