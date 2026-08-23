/*=============================================================================
	HP2MouseCapturePolicy.h: Pure decision seam for the SDL mouse-grab
	state machine (wave 3 input-contract work).
=============================================================================*/

#ifndef _INC_HP2_MOUSE_CAPTURE_POLICY
#define _INC_HP2_MOUSE_CAPTURE_POLICY

/*-----------------------------------------------------------------------------
	This header freezes the mouse-capture decisions that used to live inline
	in USDLViewport's SDL_WINDOWEVENT handler
	(HarryPotter2/Unreal/SDLDrv/Src/SDLViewport.cpp). It is deliberately free of SDL and engine includes:
	callers translate raw SDL events into the plain structs below and apply
	the returned action through UpdateMouseGrabState().

	Current truth frozen here (change only together with the table rows in
	tests/input_contracts/mouse_capture_policy of Tests/InputContractTests.cpp):

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

// What the caller must do to the grab state.
enum EAction
{
	Action_None,     // no grab-state change
	Action_Release,  // UpdateMouseGrabState(FALSE); LostGrab = 1
	Action_Restore,  // UpdateMouseGrabState(CaptureMouse config); LostGrab = 0
};

// Expected cursor-visibility effect of the transition. Informational: the
// application site gates Show/Hide on its own ActiveCursor != NULL check.
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
	if (!S.bLossPending && S.bCurrentlyGrabbed)
	{
		FDecision D = { Action_Release, Cursor_Show };
		return D;
	}
	FDecision D = { Action_None, Cursor_Leave };
	return D;
}

// Decision for one restore event (FOCUS_GAINED / RESTORED / SHOWN).
inline FDecision OnRestore(const FState& S, const FWindowFlags& W)
{
	if (S.bLossPending && W.bHasInputFocus && !W.bMinimizedOrHidden)
	{
		FDecision D = { Action_Restore, S.bCaptureMouseConfig ? Cursor_Hide : Cursor_Leave };
		return D;
	}
	FDecision D = { Action_None, Cursor_Leave };
	return D;
}

} // namespace HP2Capture

#endif //_INC_HP2_MOUSE_CAPTURE_POLICY
