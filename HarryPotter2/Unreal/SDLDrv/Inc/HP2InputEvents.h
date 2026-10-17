/*=============================================================================
	HP2InputEvents.h: Normalized raw-SDL -> engine-input-event adapter for
	the SDL driver.
=============================================================================*/

#ifndef _INC_HP2_INPUT_EVENTS
#define _INC_HP2_INPUT_EVENTS

/*-----------------------------------------------------------------------------
	Every function below is a thin forward to the Rust hp2_rs_input_*
	FFI surface (crates/hp2-platform/src/input_event.rs, exported from
	crates/hp2-ffi/src/lib.rs); the mapping logic itself lives there,
	not here. Called at USDLViewport::UpdateInput's real call sites
	(SDLViewport.cpp:345/352/370/378) and from USDLViewport's own
	constructor to fill its (never-read, kept for fill-behavior parity)
	KeysymMap field (:84-85), plus Tests/InputContractTests.cpp's
	input_event_mapping ctest, which is this cutover's real evidence:
	it calls these same functions, so a passing ctest means the Rust
	mapping is correct, not a dead C++ copy of it.

	Frozen quirks (enforced by the Rust port, not here -- do not
	reintroduce a second copy of this logic to "fix" them locally):
	both CTRL keys collapse to IK_Ctrl; both GUI (Command) keys surface
	as IK_F24; both ALT keys map to IK_Alt; the Spanish N-key (241) and
	the Apple international section sign (167) both map to IK_Tilde;
	unmapped keysyms yield IK_None and the pump suppresses those events.
-----------------------------------------------------------------------------*/

#include <SDL2/SDL.h>

// EInputKey/EInputAction (IK_*, IST_*) arrive via the engine headers, exactly
// like the sibling SDLDrv.h consumers.
#include "Engine.h"
#include "hp2_rs.h"

// Normalized input event, handed to CauseInputEvent / Engine->Key.
struct FHP2InputEvent
{
	EInputKey    Key;
	EInputAction Action;
	FLOAT        Delta;
	ANSICHAR     Text[8]; // SDL_TEXTINPUT payload copy (NUL-terminated); empty otherwise
};

// Pins the exact layout the Rust port's #[repr(C)] mirror
// (crates/hp2-platform/src/input_event.rs's HP2InputEvent) asserts via
// its own size_of/offset_of consts. A change on either side that
// breaks this contract fails that side's own build, not just at
// runtime on one machine.
static_assert( sizeof(FHP2InputEvent) == 20, "FHP2InputEvent layout must match crates/hp2-platform/src/input_event.rs's HP2InputEvent" );
static_assert( offsetof(FHP2InputEvent, Key) == 0, "FHP2InputEvent.Key offset must match HP2InputEvent.key" );
static_assert( offsetof(FHP2InputEvent, Action) == 4, "FHP2InputEvent.Action offset must match HP2InputEvent.action" );
static_assert( offsetof(FHP2InputEvent, Delta) == 8, "FHP2InputEvent.Delta offset must match HP2InputEvent.delta" );
static_assert( offsetof(FHP2InputEvent, Text) == 12, "FHP2InputEvent.Text offset must match HP2InputEvent.text" );

// FfiInputEvent (hp2_rs.h, snake_case) -> FHP2InputEvent (PascalCase).
// Same #[repr(C)] layout on the Rust side; different field names, so a
// field-by-field copy, not a reinterpret_cast.
inline FHP2InputEvent FromFfi( const FfiInputEvent& Ffi )
{
	FHP2InputEvent E;
	E.Key = (EInputKey)Ffi.key;
	E.Action = (EInputAction)Ffi.action;
	E.Delta = Ffi.delta;
	appMemcpy( E.Text, Ffi.text, (INT)sizeof(E.Text) );
	return E;
}

// Mirror of USDLViewport::KeysymMap's fill call. Returns IK_None for
// unmapped keysyms.
inline EInputKey HP2MapKeysym(SDL_Keycode Sym)
{
	return (EInputKey)hp2_rs_input_map_keysym( (INT)Sym );
}

// SDL_KEYDOWN / SDL_KEYUP -> normalized key event. Unmapped keysyms keep
// Key=IK_None so the caller can apply today's suppression rule centrally.
inline FHP2InputEvent HP2MakeKeyEvent(SDL_Keycode Sym, bool bPressed)
{
	return FromFfi( hp2_rs_input_make_key_event( (INT)Sym, bPressed ) );
}

// SDL_MOUSEBUTTONDOWN / SDL_MOUSEBUTTONUP -> normalized mouse-button event.
// Only buttons 1/2/3 reach the engine today; anything else maps to a
// suppressed IK_None event.
inline FHP2InputEvent HP2MakeMouseButtonEvent(Uint8 Button, bool bPressed)
{
	return FromFfi( hp2_rs_input_make_mouse_button_event( Button, bPressed ) );
}

// SDL_MOUSEWHEEL -> normalized wheel sequence: one IK_MouseW axis event
// carrying the wheel delta, then a synthesized press/release pair on
// IK_MouseWheelUp or IK_MouseWheelDown. Returns the number of events
// written into Out (must hold at least 3); zero when the wheel delta is
// zero, matching the pump's `if (Event.wheel.y)` guard.
inline INT HP2MakeWheelEvents(Sint32 WheelY, FHP2InputEvent Out[3])
{
	FfiInputEvent Ffi[3];
	const INT Count = hp2_rs_input_make_wheel_events( WheelY, Ffi );
	for( INT Index = 0; Index < Count; ++Index )
		Out[Index] = FromFfi( Ffi[Index] );
	return Count;
}

// SDL_TEXTINPUT -> normalized text event. The engine routes these through
// UEngine::Key (console KeyType), not InputEvent, so Action stays IST_None
// as the marker; Key carries the first decoded codepoint (IK_None when the
// payload is malformed). Text holds up to 7 payload bytes plus terminator.
inline FHP2InputEvent HP2MakeTextEvent(const char* Utf8Text)
{
	const size_t Len = Utf8Text ? strlen(Utf8Text) : 0;
	return FromFfi( hp2_rs_input_make_text_event( reinterpret_cast<const uint8_t*>(Utf8Text), Len ) );
}

#endif //_INC_HP2_INPUT_EVENTS
