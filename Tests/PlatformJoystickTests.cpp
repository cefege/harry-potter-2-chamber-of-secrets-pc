/*=============================================================================
	PlatformJoystickTests.cpp: SDL main-thread checks for
	hp2_rs_platform_open_joystick/close_joystick
	(crates/hp2-platform/src/joystick.rs), replacing
	USDLClient::OpenJoystick/CloseJoystick (SDLClient.cpp).

	No existing C++ oracle exercises these two functions (USDLClient
	member functions; Tests/InputContractTests.cpp's headless harness
	never constructs one), so this is new coverage. Unlike the other
	Platform*Tests.cpp files in this family, the device-present path
	is genuinely exercisable headless here: SDL_JoystickAttachVirtual
	(SDL2 2.0.14+, present in the pinned 2.32.10) creates a software
	joystick device with no real hardware, confirmed empirically to
	work under the dummy video driver (and does not even require video
	to be initialized) before writing this file. This test therefore
	covers the real branches derive_joystick_fields exists for --
	button clamp to 16, axis clamp to 8,
	JoyHatFoldsToButtons' threshold and IgnoreHat suppression -- not
	just the DeviceCount==0 early return a CI runner with no attached
	device would otherwise be stuck with.
	
	Also covers event translation that needs no device: keyboard and joystick
	events through hp2_rs_input_translate_event, and the release-all stepper
	through hp2_rs_input_release_all_begin/next (crates/hp2-platform/src/input_pump.rs).
	Expected values are the 710119c C++ oracle (harness scenarios keys,
	joy_axis, joy_button, hat_fold, reset_release).
=============================================================================*/

#include "SDLDrv.h"
#include "hp2_rs.h"

#include <cstdio>
#include <cstdlib>

namespace
{

void Require( bool Condition, const char* Message )
{
	if( !Condition )
	{
		std::fprintf( stderr, "PlatformJoystickTests: FAIL: %s\n", Message );
		std::exit( 1 );
	}
}

struct FVirtualJoystick
{
	int DeviceIndex;

	FVirtualJoystick( int NumAxes, int NumButtons, int NumHats )
	{
		DeviceIndex = SDL_JoystickAttachVirtual( SDL_JOYSTICK_TYPE_GAMECONTROLLER, NumAxes, NumButtons, NumHats );
	}
	~FVirtualJoystick()
	{
		if( DeviceIndex >= 0 )
			SDL_JoystickDetachVirtual( DeviceIndex );
	}
};

// Oracle context: the harness constants (JoystickDeadZone=0.1f, ScaleJBX=2.f,
// ScaleJBY=0.5f, MouseScaleX=0.5f, MouseScaleY=0.25f, SizeX=640, SizeY=480).
FfiInputContext MakeContext()
{
	FfiInputContext Ctx{};
	Ctx.mouse_is_grabbed = false;
	Ctx.lost_grab = false;
	Ctx.capture_mouse = false;
	Ctx.has_window = true;
	Ctx.has_engine = true;
	Ctx.has_actor = true;
	Ctx.joy_hat_folds_to_buttons = true;
	Ctx.mouse_scale_x = 0.5f;
	Ctx.mouse_scale_y = 0.25f;
	Ctx.size_x = 640;
	Ctx.size_y = 480;
	Ctx.joystick_dead_zone = 0.1f;
	Ctx.scale_jbx = 2.f;
	Ctx.scale_jby = 0.5f;
	Ctx.last_joy_hat = 0;
	return Ctx;
}

// Mirrors MAX_ACTIONS in crates/hp2-platform/src/input_pump.rs.
const int MaxActions = 4;

int Translate( const FfiInputContext& Ctx, const FfiSdlEvent& Ev, FfiInputAction* Out )
{
	return hp2_rs_input_translate_event( &Ctx, &Ev, Out );
}

bool IsCause( const FfiInputAction& Step, int Key, int Action, float Delta )
{
	return Step.kind == InputActionCause && Step.key == Key && Step.action == Action && Step.delta == Delta;
}

bool HasCause( const FfiInputAction* Out, int Count, int Key, int Action, float Delta )
{
	for( int Index = 0; Index < Count; ++Index )
		if( IsCause( Out[Index], Key, Action, Delta ) )
			return true;
	return false;
}

struct FReleaseTrace
{
	FfiInputAction Causes[ 32 ];
	int CauseCount = 0;
	int QueryCount = 0;
};

// Drives the release stepper the way SDLViewport.cpp's ReleaseAllInput does.
// QueryKeyDown steps are answered from Held; the answer rides on the next call.
FReleaseTrace TraceReleaseAll( bool HasInput, const int* Held, int HeldCount )
{
	FReleaseTrace Trace;
	FfiReleaseAllCursor Cursor = hp2_rs_input_release_all_begin( HasInput );
	bool bHeld = false;
	FfiInputAction Step{};
	while( hp2_rs_input_release_all_next( &Cursor, bHeld, &Step ) )
	{
		bHeld = false;
		if( Step.kind == InputActionQueryKeyDown )
		{
			++Trace.QueryCount;
			for( int Index = 0; Index < HeldCount; ++Index )
				if( Held[Index] == Step.key )
					bHeld = true;
		}
		else if( Step.kind == InputActionCause )
		{
			Require( Trace.CauseCount < 32, "release_all emitted more Cause steps than the oracle allows" );
			Trace.Causes[Trace.CauseCount++] = Step;
		}
	}
	return Trace;
}


} // namespace

int main()
{
	Require( SDL_InitSubSystem( SDL_INIT_JOYSTICK ) == 0, "SDL_InitSubSystem(SDL_INIT_JOYSTICK) failed" );

	// Out-of-range JoystickNumber (no devices attached at all yet):
	// must fail cleanly, matching the real C++'s `JoystickNumber <
	// 0 || JoystickNumber >= DeviceCount` guard.
	{
		INT Buttons = -1, Hats = -1, Axes = -1;
		bool bHatFoldsToButtons = true; // deliberately wrong initial value
		void* Handle = hp2_rs_platform_open_joystick( 0, false, &Buttons, &Hats, &Axes, &bHatFoldsToButtons );
		Require( Handle == NULL, "expected open_joystick to fail with no devices attached" );
		// Out-params must be left untouched on failure, matching the
		// real C++'s own `return 0` before touching JoyButtons etc.
		Require( Buttons == -1 && Hats == -1 && Axes == -1 && bHatFoldsToButtons == true, "out-params must be untouched on failure" );
	}

	// 14 buttons + 1 hat, IgnoreHat=false -> clamp is a no-op (14<=16),
	// folding engages (14>12, 1 hat, IgnoreHat off).
	{
		FVirtualJoystick Joy( /*axes=*/4, /*buttons=*/14, /*hats=*/1 );
		Require( Joy.DeviceIndex >= 0, "SDL_JoystickAttachVirtual failed" );

		INT Buttons = 0, Hats = 0, Axes = 0;
		bool bHatFoldsToButtons = false;
		void* Handle = hp2_rs_platform_open_joystick( Joy.DeviceIndex, false, &Buttons, &Hats, &Axes, &bHatFoldsToButtons );
		Require( Handle != NULL, "expected open_joystick to succeed against the virtual device" );
		Require( Buttons == 14, "expected 14 buttons, no clamp needed" );
		Require( Hats == 1, "expected 1 hat" );
		Require( Axes == 4, "expected 4 axes, no clamp needed" );
		Require( bHatFoldsToButtons == true, "expected hat folding: 14>12 buttons, 1 hat, IgnoreHat off" );

		hp2_rs_platform_close_joystick( Handle );
	}

	// 12 buttons + 1 hat: folding must NOT engage (threshold is
	// strictly >12, not >=12).
	{
		FVirtualJoystick Joy( /*axes=*/2, /*buttons=*/12, /*hats=*/1 );
		Require( Joy.DeviceIndex >= 0, "SDL_JoystickAttachVirtual failed" );

		INT Buttons = 0, Hats = 0, Axes = 0;
		bool bHatFoldsToButtons = true;
		void* Handle = hp2_rs_platform_open_joystick( Joy.DeviceIndex, false, &Buttons, &Hats, &Axes, &bHatFoldsToButtons );
		Require( Handle != NULL, "expected open_joystick to succeed against the virtual device" );
		Require( bHatFoldsToButtons == false, "expected no hat folding at exactly 12 buttons (threshold is >12)" );

		hp2_rs_platform_close_joystick( Handle );
	}

	// 14 buttons + 1 hat, IgnoreHat=true -> folding suppressed despite
	// otherwise qualifying.
	{
		FVirtualJoystick Joy( /*axes=*/2, /*buttons=*/14, /*hats=*/1 );
		Require( Joy.DeviceIndex >= 0, "SDL_JoystickAttachVirtual failed" );

		INT Buttons = 0, Hats = 0, Axes = 0;
		bool bHatFoldsToButtons = true;
		void* Handle = hp2_rs_platform_open_joystick( Joy.DeviceIndex, true, &Buttons, &Hats, &Axes, &bHatFoldsToButtons );
		Require( Handle != NULL, "expected open_joystick to succeed against the virtual device" );
		Require( bHatFoldsToButtons == false, "expected IgnoreHat to suppress folding" );

		hp2_rs_platform_close_joystick( Handle );
	}

	// 20 buttons, 10 axes, 2 hats: both clamps must engage
	// (buttons->16, axes->8); folding still applies.
	{
		FVirtualJoystick Joy( /*axes=*/10, /*buttons=*/20, /*hats=*/2 );
		Require( Joy.DeviceIndex >= 0, "SDL_JoystickAttachVirtual failed" );

		INT Buttons = 0, Hats = 0, Axes = 0;
		bool bHatFoldsToButtons = false;
		void* Handle = hp2_rs_platform_open_joystick( Joy.DeviceIndex, false, &Buttons, &Hats, &Axes, &bHatFoldsToButtons );
		Require( Handle != NULL, "expected open_joystick to succeed against the virtual device" );
		Require( Buttons == 16, "expected buttons clamped to 16" );
		Require( Hats == 2, "expected 2 hats, no clamp applies to hats" );
		Require( Axes == 8, "expected axes clamped to 8" );
		Require( bHatFoldsToButtons == true, "expected hat folding: 20>12 buttons (pre-clamp), hats present" );

		hp2_rs_platform_close_joystick( Handle );
	}

	// Keyboard translation through hp2_rs_input_translate_event (oracle: keys).
	// SDLK_a press/release is key 65 (action 1/3); SDLK_UNKNOWN emits nothing.
	{
		const FfiInputContext Ctx = MakeContext();
		FfiSdlEvent Ev{};
		FfiInputAction Out[ MaxActions ]{};
		Ev.kind = SDL_KEYDOWN;
		Ev.key_sym = SDLK_a;
		const int Down = Translate( Ctx, Ev, Out );
		Require( HasCause( Out, Down, 65, 1, 0.f ), "SDLK_a down must press key 65" );
		Ev.kind = SDL_KEYUP;
		const int Up = Translate( Ctx, Ev, Out );
		Require( HasCause( Out, Up, 65, 3, 0.f ), "SDLK_a up must release key 65" );
		Ev.kind = SDL_KEYDOWN;
		Ev.key_sym = SDLK_UNKNOWN;
		Require( Translate( Ctx, Ev, Out ) == 0, "SDLK_UNKNOWN must emit nothing" );
	}

	// Joystick event mapping through hp2_rs_input_translate_event. These replace
	// the removed hp2_rs_input_map_joystick_axis/button and fold_joystick_hat
	// checks (oracle: joy_axis, joy_button, hat_fold).
	{
		const FfiInputContext Ctx = MakeContext();
		FfiSdlEvent Ev{};
		FfiInputAction Out[ MaxActions ]{};
		Ev.kind = SDL_JOYAXISMOTION;
		Ev.joy_axis = 0;
		Ev.joy_axis_value = 16384;
		Require( Translate( Ctx, Ev, Out ) == 1 && IsCause( Out[0], 224, 4, 1.f ),
			"axis 0 at 16384 must map to key 224 with delta 1.0" );
		Ev.joy_axis = 1;
		Ev.joy_axis_value = -32768;
		Require( Translate( Ctx, Ev, Out ) == 1 && IsCause( Out[0], 225, 4, -0.5f ),
			"axis 1 at -32768 must map to key 225 with delta -0.5" );
		Ev.joy_axis = 2;
		Ev.joy_axis_value = 1000;
		Require( Translate( Ctx, Ev, Out ) == 1 && IsCause( Out[0], 226, 4, 0.f ),
			"axis 2 inside the dead zone must emit delta 0" );
		Ev.joy_axis = 8;
		Ev.joy_axis_value = 16384;
		Require( Translate( Ctx, Ev, Out ) == 0, "axis 8 is out of range and must be dropped" );

		Ev = FfiSdlEvent{};
		Ev.kind = SDL_JOYBUTTONDOWN;
		Ev.joy_button = 3;
		Require( Translate( Ctx, Ev, Out ) == 1 && IsCause( Out[0], 203, 1, 0.f ),
			"button 3 down must press key 203" );
		Ev.joy_button = 16;
		Require( Translate( Ctx, Ev, Out ) == 0, "button 16 is out of range and must be dropped" );
		Ev.kind = SDL_JOYBUTTONUP;
		Ev.joy_button = 0;
		Require( Translate( Ctx, Ev, Out ) == 1 && IsCause( Out[0], 200, 3, 0.f ),
			"button 0 up must release key 200" );

		FfiInputContext HatCtx = MakeContext();
		Ev = FfiSdlEvent{};
		Ev.kind = SDL_JOYHATMOTION;
		Ev.joy_hat = SDL_HAT_UP | SDL_HAT_RIGHT;
		const int Pressed = Translate( HatCtx, Ev, Out );
		Require( Pressed >= 1 && HasCause( Out, Pressed, 212, 1, 0.f ), "UP|RIGHT must press key 212" );
		Require( Out[Pressed - 1].kind == InputActionSetLastJoyHat, "a hat press must end with SetLastJoyHat" );
		HatCtx.last_joy_hat = 212;
		Ev.joy_hat = SDL_HAT_CENTERED;
		const int Released = Translate( HatCtx, Ev, Out );
		Require( Released >= 1 && HasCause( Out, Released, 212, 3, 0.f ), "centered hat must release key 212" );
	}

	// Release-all through hp2_rs_input_release_all_begin/next (oracle:
	// reset_release, release_all_empty). Held keys 16 (IK_Shift), 65 (IK_A) and
	// 200 (IK_Joy1) release first in key order, then the joystick axes and mouse.
	{
		const int Held[] = { 16, 65, 200 };
		const FReleaseTrace Trace = TraceReleaseAll( true, Held, 3 );
		Require( Trace.QueryCount == IK_MAX, "release_all must query every key (IK_MAX) before the tail" );
		const int Expect[] = { 16, 65, 200, 224, 225, 226, 227, 232, 233, 234, 235, 228, 229, 231 };
		Require( Trace.CauseCount == 14, "release_all with held keys must emit 14 Cause steps" );
		for( int Index = 0; Index < Trace.CauseCount; ++Index )
		{
			Require( Trace.Causes[Index].key == Expect[Index], "release_all key order drifted from the oracle" );
			Require( Trace.Causes[Index].action == ( Index < 3 ? 3 : 4 ), "release_all action drifted from the oracle" );
		}
	}
	{
		const FReleaseTrace Trace = TraceReleaseAll( true, NULL, 0 );
		const int Expect[] = { 224, 225, 226, 227, 232, 233, 234, 235, 228, 229, 231 };
		Require( Trace.QueryCount == IK_MAX, "release_all with no held keys must query every key (IK_MAX)" );
		Require( Trace.CauseCount == 11, "release_all with no held keys must emit the axis and mouse tail only" );
		for( int Index = 0; Index < Trace.CauseCount; ++Index )
			Require( Trace.Causes[Index].key == Expect[Index] && Trace.Causes[Index].action == 4,
				"release_all tail drifted from the oracle" );
	}

	// Null-handle close must no-op safely (matches the real C++'s own
	// `if (Joystick)` guard).
	hp2_rs_platform_close_joystick( NULL );

	std::puts( "PlatformJoystickTests: PASS" );
	return 0;
}
