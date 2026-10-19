// Fixture driver for the ORIGINAL UnPlayer.cpp (UPlayer::Destroy and UPlayer::Exec) at the
// pinned ref. Engine objects are calloc'd and given hand-built vtables (levact_harness
// pattern); only the virtual callees UPlayer reaches are forwarded to a trace recorder.
// Each record prints the scenario inputs, the ORIGINAL return value, the ORIGINAL ordered
// trace of callee invocations (callee, executor nullness, callee return value) and the
// final Actor state. The Rust replay drives the ported logic against a mock that emits the
// same tokens.
//
// Token grammar: LX<r> ULevel::Exec, EX<r> UEngine::Exec, {AS,HS,GS}<e><r> actor/HUD/game
// ScriptConsoleExec, I<k>S<e><r> inventory item k ScriptConsoleExec, DA<force><isActor>
// ULevel::DestroyActor. <e> is 1 when the executor argument is non-null.
#include "EnginePrivate.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <string>

// Defined here so the stub generator leaves it alone; the engine global lives in Core.
UBOOL GIsRunning = 0;

static void* g_sce_vt[512];
static void* g_lvl_vt[512];
static void* g_eng_vt[512];

// Vtable slot of a virtual member function pointer (ARM member-pointer ABI: word0 is the
// byte offset of the vtable entry, word1 bit0 marks a virtual member).
static size_t vt_slot( const void* Pm )
{
	uintptr_t W[2];
	memcpy( W, Pm, sizeof W );
	if( !(W[1] & 1) )
		abort();
	return W[0] / sizeof(void*);
}
static void set_vptr( void* Obj, void** Vt )
{
	memcpy( Obj, &Vt, sizeof Vt );
}

// Base constructors are never run (objects are calloc'd), so the UObject destructor-side
// Super::Destroy target is a no-op here; the Rust side keeps Super::Destroy in C++.
void UObject::Destroy() {}

static std::string g_trace;
static void tr( const char* Tok )
{
	if( !g_trace.empty() )
		g_trace += ',';
	g_trace += Tok;
}

static UPlayer*    g_player;
static APlayerPawn* g_actor;
static ULevel*     g_lvl;
static ALevelInfo* g_li;
static AGameInfo*  g_game;
static AHUD*       g_hud;
static AInventory* g_inv[3];
static UEngine*    g_eng;
static unsigned char g_ar_buf[256];

// Scenario knobs read by the forwarders.
// g_mut: 0 none, 1 ULevel::Exec, 2 game, 3 HUD, 4 actor, 5 first inventory item.
// A mutation clears the player's Actor before the callee returns, which must make the
// ORIGINAL skip every later step; the Rust port must re-read Actor at each step to match.
static int g_mut, g_lvl_ret, g_game_ret, g_hud_ret, g_act_ret, g_inv_bits, g_eng_ret;

static UBOOL fwd_sce( UObject* Self, const TCHAR* Cmd, FOutputDevice& Ar, UObject* Executor )
{
	char tok[32];
	int ret, mut = -1;
	if( (void*)Self == (void*)g_actor )
	{
		ret = g_act_ret; mut = 4;
		snprintf( tok, sizeof tok, "AS%d%d", Executor != NULL, ret );
	}
	else if( (void*)Self == (void*)g_hud )
	{
		ret = g_hud_ret; mut = 3;
		snprintf( tok, sizeof tok, "HS%d%d", Executor != NULL, ret );
	}
	else if( (void*)Self == (void*)g_game )
	{
		ret = g_game_ret; mut = 2;
		snprintf( tok, sizeof tok, "GS%d%d", Executor != NULL, ret );
	}
	else
	{
		int k = -1;
		for( int i = 0; i < 3; i++ )
			if( (void*)Self == (void*)g_inv[i] )
				k = i;
		if( k < 0 )
			abort();
		ret = (g_inv_bits >> k) & 1;
		if( k == 0 )
			mut = 5;
		snprintf( tok, sizeof tok, "I%dS%d%d", k, Executor != NULL, ret );
	}
	tr( tok );
	if( mut >= 0 && g_mut == mut )
		g_player->Actor = NULL;
	return ret;
}

static UBOOL fwd_lvl_exec( ULevel* Self, const TCHAR* Cmd, FOutputDevice& Ar )
{
	char tok[16];
	snprintf( tok, sizeof tok, "LX%d", g_lvl_ret );
	tr( tok );
	if( g_mut == 1 )
		g_player->Actor = NULL;
	return g_lvl_ret;
}

static UBOOL fwd_eng_exec( UEngine* Self, const TCHAR* Cmd, FOutputDevice& Ar )
{
	char tok[16];
	snprintf( tok, sizeof tok, "EX%d", g_eng_ret );
	tr( tok );
	return g_eng_ret;
}

static UBOOL fwd_destroy( ULevel* Self, AActor* Actor, UBOOL bNetForce )
{
	char tok[16];
	snprintf( tok, sizeof tok, "DA%d%d", (int)bNetForce, (void*)Actor == (void*)g_actor );
	tr( tok );
	return 1;
}

static void apply_common( int Act, int Li, int Game, int Hud, int Ninv, int Ibits )
{
	g_actor->XLevel    = g_lvl;
	g_lvl->Engine      = g_eng;
	g_actor->Level     = Li ? g_li : NULL;
	g_li->Game         = Game ? g_game : NULL;
	g_actor->myHUD     = Hud ? g_hud : NULL;
	g_actor->Inventory = Ninv > 0 ? g_inv[0] : NULL;
	for( int k = 0; k < 3; k++ )
		g_inv[k]->Inventory = (k + 1 < Ninv) ? g_inv[k + 1] : NULL;
	g_inv_bits = Ibits;
	g_player->Actor = Act ? g_actor : NULL;
	g_actor->Player = g_player;
	g_trace.clear();
}

static uint32_t g_rng = 0x2545F491u;
static uint32_t rnd()
{
	g_rng = g_rng * 1664525u + 1013904223u;
	return g_rng >> 8;
}

int main()
{
	auto PmSce = &UObject::ScriptConsoleExec;
	auto PmLExec = &ULevel::Exec;
	auto PmLDestroy = &ULevel::DestroyActor;
	auto PmEExec = &UEngine::Exec;
	g_sce_vt[vt_slot( &PmSce )] = (void*)&fwd_sce;
	g_lvl_vt[vt_slot( &PmLExec )] = (void*)&fwd_lvl_exec;
	g_lvl_vt[vt_slot( &PmLDestroy )] = (void*)&fwd_destroy;
	g_eng_vt[vt_slot( &PmEExec )] = (void*)&fwd_eng_exec;

	g_player = (UPlayer*)calloc( 1, sizeof(UPlayer) );
	g_actor  = (APlayerPawn*)calloc( 1, sizeof(APlayerPawn) );
	g_lvl    = (ULevel*)calloc( 1, sizeof(ULevel) );
	g_li     = (ALevelInfo*)calloc( 1, sizeof(ALevelInfo) );
	g_game   = (AGameInfo*)calloc( 1, sizeof(AGameInfo) );
	g_hud    = (AHUD*)calloc( 1, sizeof(AHUD) );
	g_eng    = (UEngine*)calloc( 1, sizeof(UEngine) );
	for( int k = 0; k < 3; k++ )
		g_inv[k] = (AInventory*)calloc( 1, sizeof(AInventory) );
	set_vptr( g_actor, g_sce_vt );
	set_vptr( g_game, g_sce_vt );
	set_vptr( g_hud, g_sce_vt );
	for( int k = 0; k < 3; k++ )
		set_vptr( g_inv[k], g_sce_vt );
	set_vptr( g_lvl, g_lvl_vt );
	set_vptr( g_eng, g_eng_vt );

	FOutputDevice& Ar = *(FOutputDevice*)g_ar_buf;
	const TCHAR* Cmd = TEXT("SCRIPT CMD");

	printf( "# player fixture: ORIGINAL UnPlayer.cpp @ 5782e7f, generated by gen_player.sh\n" );
	printf( "# exec fields: act li game hud ninv ibits lvl mut gr hr acr er -> ret final trace\n" );
	for( int c = 0; c < 600; c++ )
	{
		int act  = (rnd() % 5) != 0;
		int lvl  = rnd() & 1;
		int mut  = rnd() % 6;
		int li   = (rnd() % 4) != 0;
		int game = (rnd() % 4) != 0;
		int gr   = rnd() & 1;
		int hud  = rnd() & 1;
		int hr   = rnd() & 1;
		int acr  = (rnd() % 3) == 0 ? 1 : 0;
		int ninv = rnd() % 4;
		int ibits = rnd() & 7;
		int er   = rnd() & 1;
		g_lvl_ret = lvl; g_mut = mut; g_game_ret = gr; g_hud_ret = hr;
		g_act_ret = acr; g_eng_ret = er;
		apply_common( act, li, game, hud, ninv, ibits );
		UBOOL ret = g_player->UPlayer::Exec( Cmd, Ar );
		printf( "exec act=%d li=%d game=%d hud=%d ninv=%d ibits=%d lvl=%d mut=%d gr=%d hr=%d acr=%d er=%d -> ret=%d final=%d trace=%s\n",
			act, li, game, hud, ninv, ibits, lvl, mut, gr, hr, acr, er,
			(int)ret, g_player->Actor != NULL, g_trace.empty() ? "-" : g_trace.c_str() );
	}

	printf( "# destroy fields: run act -> pnull panull trace\n" );
	for( int run = 0; run < 2; run++ )
		for( int act = 0; act < 2; act++ )
		{
			apply_common( act, 1, 1, 1, 0, 0 );
			g_mut = 0;
			GIsRunning = run;
			g_player->UPlayer::Destroy();
			printf( "destroy run=%d act=%d -> pnull=%d panull=%d trace=%s\n",
				run, act, g_actor->Player == NULL, g_player->Actor == NULL,
				g_trace.empty() ? "-" : g_trace.c_str() );
		}
	return 0;
}
