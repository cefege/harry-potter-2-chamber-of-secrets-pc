/*=============================================================================
	HP2MouseCapturePolicy.h: Pure decision seam for the SDL mouse-grab
	state machine.
=============================================================================*/

#ifndef _INC_HP2_MOUSE_CAPTURE_POLICY
#define _INC_HP2_MOUSE_CAPTURE_POLICY

#include "hp2_rs.h"

/*-----------------------------------------------------------------------------
	`OnLoss`/`OnRestore` are thin forwards to the Rust
	hp2_rs_mouse_capture_* FFI surface
	(crates/hp2-platform/src/mouse_capture.rs, exported from
	crates/hp2-ffi/src/lib.rs); the decision logic itself lives there,
	not here. Called from USDLViewport's SDL_WINDOWEVENT handler
	(SDLViewport.cpp:434/451) and from Tests/InputContractTests.cpp's
	mouse_capture_policy ctest, which is this cutover's real evidence:
	it calls these same functions, so a passing ctest means the Rust
	decision table is correct, not a dead C++ copy of it.

	This header stays deliberately free of SDL and engine includes
	(besides hp2_rs.h): callers translate raw SDL events into the
	plain structs below and apply the returned action through
	UpdateMouseGrabState().

	Current truth (change only together with the table rows in
	tests/input_contracts/mouse_capture_policy of
	Tests/InputContractTests.cpp, and crates/hp2-platform/src/
	mouse_capture.rs's own test table, which the oracle is mirrored
	against):

	  - Loss events are SDL_WINDOWEVENT_FOCUS_LOST, _MINIMIZED and _HIDDEN.
	    The grab is released only on the FIRST loss while grabbed; repeated
	    loss events while a loss is already pending leave capture state
	    alone. ReleaseAllInput() still runs unconditionally at the call
	    site on every loss event - that behavior is NOT part of this seam.

	  - Restore events are SDL_WINDOWEVENT_FOCUS_GAINED, _RESTORED and
	    _SHOWN. The grab returns only when a loss is pending AND SDL
	    reports input focus AND the window is neither minimized nor hidden.
	    The restored grab follows the CaptureMouse configuration rather
	    than the pre-loss state.

	  - Menu state (USDLClient::InMenuLoop) deliberately does NOT take part:
	    today's driver never consults it when grabbing or releasing. This is
	    a documented gap, not a policy input (see Tests/InputContractTests.cpp).

	  - Cursor visibility rides the transition inside UpdateMouseGrabState():
	    releasing shows the cursor, grabbing hides it again. Restoring with
	    CaptureMouse=False keeps the window ungrabbed, so UpdateMouseGrabState
	    sees no transition at all and the cursor simply stays visible from the
	    earlier release - encoded below as Cursor_Leave for that case.
-----------------------------------------------------------------------------*/

namespace HP2Capture {

// What the caller must do to the grab state. Values match the Rust
// port's Action enum (crates/hp2-platform/src/mouse_capture.rs)
// exactly -- both default-numbered, pinned explicitly on the Rust
// side since the value crosses this FFI boundary.
enum EAction
{
	Action_None,     // no grab-state change
	Action_Release,  // UpdateMouseGrabState(FALSE); LostGrab = 1
	Action_Restore,  // UpdateMouseGrabState(CaptureMouse config); LostGrab = 0
};

// Expected cursor-visibility effect of the transition. Informational: the
// application site gates Show/Hide on its own ActiveCursor != NULL check.
// Values match the Rust port's Cursor enum exactly (Hide=-1, Leave=0,
// Show=1, pinned explicitly on the Rust side since these are non-default
// enum values that cross this FFI boundary).
enum ECursor
{
	Cursor_Hide  = -1, // transition expects the cursor to become hidden
	Cursor_Leave =  0, // no cursor-visibility change expected
	Cursor_Show  =  1, // transition expects the cursor to become visible
};

struct FState
{
	bool bCurrentlyGrabbed;   // USDLViewport::MouseIsGrabbed before the event
	bool bLossPending;        // USDLViewport::LostGrab before the event
	bool bCaptureMouseConfig; // UClient::CaptureMouse configuration
};

struct FWindowFlags
{
	bool bHasInputFocus;      // SDL_WINDOW_INPUT_FOCUS set
	bool bMinimizedOrHidden;  // SDL_WINDOW_MINIMIZED or SDL_WINDOW_HIDDEN set
};

struct FDecision
{
	EAction Action;
	ECursor Cursor;
};

// Decision for one loss event (FOCUS_LOST / MINIMIZED / HIDDEN).
inline FDecision OnLoss(const FState& S)
{
	const FfiCaptureDecision Ffi = hp2_rs_mouse_capture_on_loss( S.bCurrentlyGrabbed, S.bLossPending, S.bCaptureMouseConfig );
	FDecision D = { (EAction)Ffi.action, (ECursor)Ffi.cursor };
	return D;
}

// Decision for one restore event (FOCUS_GAINED / RESTORED / SHOWN).
inline FDecision OnRestore(const FState& S, const FWindowFlags& W)
{
	const FfiCaptureDecision Ffi = hp2_rs_mouse_capture_on_restore(
		S.bCurrentlyGrabbed, S.bLossPending, S.bCaptureMouseConfig,
		W.bHasInputFocus, W.bMinimizedOrHidden );
	FDecision D = { (EAction)Ffi.action, (ECursor)Ffi.cursor };
	return D;
}

} // namespace HP2Capture

#endif //_INC_HP2_MOUSE_CAPTURE_POLICY
