#ifndef _INC_HP2PLAYERSERVICES_H
#define _INC_HP2PLAYERSERVICES_H

// C callbacks the Rust UnPlayer port (crates/hp2-engine/src/player.rs) calls.
// Field order must match `PlayerServices` on the Rust side.
// Object pointers are typed `void` here so this header stays free of Engine includes.
struct Hp2PlayerServices
{
	// GIsRunning
	int   (*is_running)( void );
	// UPlayer::Actor
	void* (*player_actor)( void* Player );
	// UPlayer::Actor = Actor
	void  (*player_set_actor)( void* Player, void* Actor );
	// AActor::GetLevel()
	void* (*actor_get_level)( void* Actor );
	// AActor::Player = NULL
	void  (*actor_clear_player)( void* Actor );
	// ULevel::DestroyActor(Actor, bNetForce)
	void  (*level_destroy_actor)( void* Level, void* Actor, int Force );
	// ULevel::Exec(Cmd, Ar)
	int   (*level_exec)( void* Level, const void* Cmd, void* Ar );
	// AActor::Level
	void* (*actor_level)( void* Actor );
	// ALevelInfo::Game
	void* (*level_game)( void* LevelInfo );
	// UObject::ScriptConsoleExec(Cmd, Ar, Executor)
	int   (*object_script_console_exec)( void* Obj, const void* Cmd, void* Ar, void* Executor );
	// AActor::myHUD
	void* (*actor_hud)( void* Actor );
	// AActor::Inventory
	void* (*actor_inventory)( void* Actor );
	// AInventory::Inventory
	void* (*inventory_next)( void* Inv );
	// ULevel::Engine
	void* (*level_engine)( void* Level );
	// UEngine::Exec(Cmd, Ar)
	int   (*engine_exec)( void* Engine, const void* Cmd, void* Ar );
};

extern const Hp2PlayerServices GHp2PlayerServices;

#endif // _INC_HP2PLAYERSERVICES_H
