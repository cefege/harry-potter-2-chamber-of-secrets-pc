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
	button clamp to 16, axis clamp to MaxJoystickAxes (8),
	JoyHatFoldsToButtons' threshold and IgnoreHat suppression -- not
	just the DeviceCount==0 early return a CI runner with no attached
	device would otherwise be stuck with.
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
	// (buttons->16, axes->MaxJoystickAxes=8); folding still applies.
	{
		FVirtualJoystick Joy( /*axes=*/10, /*buttons=*/20, /*hats=*/2 );
		Require( Joy.DeviceIndex >= 0, "SDL_JoystickAttachVirtual failed" );

		INT Buttons = 0, Hats = 0, Axes = 0;
		bool bHatFoldsToButtons = false;
		void* Handle = hp2_rs_platform_open_joystick( Joy.DeviceIndex, false, &Buttons, &Hats, &Axes, &bHatFoldsToButtons );
		Require( Handle != NULL, "expected open_joystick to succeed against the virtual device" );
		Require( Buttons == 16, "expected buttons clamped to 16" );
		Require( Hats == 2, "expected 2 hats, no clamp applies to hats" );
		Require( Axes == 8, "expected axes clamped to MaxJoystickAxes (8)" );
		Require( bHatFoldsToButtons == true, "expected hat folding: 20>12 buttons (pre-clamp), hats present" );

		hp2_rs_platform_close_joystick( Handle );
	}

	// Null-handle close must no-op safely (matches the real C++'s own
	// `if (Joystick)` guard).
	hp2_rs_platform_close_joystick( NULL );

	// hp2_rs_input_map_joystick_axis/map_joystick_button/
	// fold_joystick_hat (crates/hp2-platform/src/joystick_input.rs):
	// the underlying logic already has its own Rust unit tests; these
	// checks exist to prove the FFI boundary itself -- argument
	// marshaling, FfiKeyAction's layout -- not to re-litigate the
	// logic.
	{
		INT Key = -1; FLOAT Value = -1.f;
		Require( hp2_rs_input_map_joystick_axis( 0, 16384, 0.f, 1.f, 1.f, &Key, &Value ), "axis 0 should map" );
		Require( Key == IK_JoyX, "axis 0 should map to IK_JoyX" );
		Require( Value == 0.5f, "raw value 16384 should scale to exactly 0.5" );
		Require( !hp2_rs_input_map_joystick_axis( 8, 0, 0.f, 1.f, 1.f, &Key, &Value ), "axis 8 is out of range" );
	}
	{
		INT Key = -1, Action = -1;
		Require( hp2_rs_input_map_joystick_button( 0, true, &Key, &Action ), "button 0 should map" );
		Require( Key == IK_Joy1 && Action == IST_Press, "button 0 pressed should map to IK_Joy1/IST_Press" );
		Require( !hp2_rs_input_map_joystick_button( 16, true, &Key, &Action ), "button 16 is out of range" );
	}
	{
		FfiKeyAction Events[2];
		INT NewLastJoyHat = -1;
		const INT Count = hp2_rs_input_fold_joystick_hat( SDL_HAT_UP, IK_None, Events, &NewLastJoyHat );
		Require( Count == 1, "first hat press should produce one event" );
		Require( NewLastJoyHat == IK_Joy1 + 12, "SDL_HAT_UP should fold to IK_Joy1+12" );
		Require( Events[0].key == IK_Joy1 + 12 && Events[0].action == IST_Press, "expected a press event" );
	}
	{
		// Direction change: UP -> DOWN must release the old folded key
		// and press the new one, exercising the array stride
		// (out_events.add(i)) and Events[1], not just Events[0].
		FfiKeyAction Events[2];
		INT NewLastJoyHat = -1;
		const INT Count = hp2_rs_input_fold_joystick_hat( SDL_HAT_DOWN, IK_Joy1 + 12, Events, &NewLastJoyHat );
		Require( Count == 2, "a direction change should produce a release and a press" );
		Require( NewLastJoyHat == IK_Joy1 + 13, "SDL_HAT_DOWN should fold to IK_Joy1+13" );
		Require( Events[0].key == IK_Joy1 + 12 && Events[0].action == IST_Release, "Events[0] should release the old direction" );
		Require( Events[1].key == IK_Joy1 + 13 && Events[1].action == IST_Press, "Events[1] should press the new direction" );
	}

	std::puts( "PlatformJoystickTests: PASS" );
	return 0;
}
