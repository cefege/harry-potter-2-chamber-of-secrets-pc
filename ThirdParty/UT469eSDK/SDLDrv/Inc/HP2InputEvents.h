/*=============================================================================
	HP2InputEvents.h: Normalized input-event adapter for the SDL driver
	(PREP ONLY - not yet wired into the viewport pump).
=============================================================================*/

#ifndef _INC_HP2_INPUT_EVENTS
#define _INC_HP2_INPUT_EVENTS

/*-----------------------------------------------------------------------------
	PREP ONLY (wave 3): this adapter defines the normalized event shape and
	the pure raw-SDL -> normalized mapping helpers. The USDLViewport event
	pump is deliberately NOT rewired to it this wave; adoption is a follow-up
	gated on the input freeze contracts (tests/input_contracts/* in
	Tests/InputContractTests.cpp) being green.

	Divergence duty: HP2MapKeysym mirrors the KeysymMap table built in the
	USDLViewport constructor. Until adoption replaces that table, the two
	must be kept identical; the mapping tests freeze the representative rows
	and any future drift must update both together.

	Frozen quirks mirrored from KeysymMap (do not "fix" here unilaterally):
	  - Both CTRL keys collapse to IK_Ctrl; both GUI (Command) keys surface
	    as IK_F24; both ALT keys map to IK_Alt.
	  - The Spanish N-key (241) and the Apple international section sign
	    (167) both map to IK_Tilde.
	  - Unmapped keysyms yield IK_None and the pump suppresses those events.
-----------------------------------------------------------------------------*/

#include <SDL2/SDL.h>

// EInputKey/EInputAction (IK_*, IST_*) arrive via the engine headers, exactly
// like the sibling SDLDrv.h consumers.
#include "Engine.h"

// Normalized input event: the single shape the follow-up pump rewrite will
// hand to CauseInputEvent / Engine->Key.
struct FHP2InputEvent
{
	EInputKey    Key;
	EInputAction Action;
	FLOAT        Delta;
	ANSICHAR     Text[8]; // SDL_TEXTINPUT payload copy (NUL-terminated); empty otherwise
};

inline FHP2InputEvent HP2MakeNoneEvent()
{
	FHP2InputEvent E;
	E.Key = IK_None; E.Action = IST_None; E.Delta = 0.f; E.Text[0] = 0;
	return E;
}

// Mirror of USDLViewport::KeysymMap. Returns IK_None for unmapped keysyms.
inline EInputKey HP2MapKeysym(SDL_Keycode Sym)
{
	switch (Sym)
	{
		// Letters.
		case SDLK_a: return IK_A;
		case SDLK_b: return IK_B;
		case SDLK_c: return IK_C;
		case SDLK_d: return IK_D;
		case SDLK_e: return IK_E;
		case SDLK_f: return IK_F;
		case SDLK_g: return IK_G;
		case SDLK_h: return IK_H;
		case SDLK_i: return IK_I;
		case SDLK_j: return IK_J;
		case SDLK_k: return IK_K;
		case SDLK_l: return IK_L;
		case SDLK_m: return IK_M;
		case SDLK_n: return IK_N;
		case SDLK_o: return IK_O;
		case SDLK_p: return IK_P;
		case SDLK_q: return IK_Q;
		case SDLK_r: return IK_R;
		case SDLK_s: return IK_S;
		case SDLK_t: return IK_T;
		case SDLK_u: return IK_U;
		case SDLK_v: return IK_V;
		case SDLK_w: return IK_W;
		case SDLK_x: return IK_X;
		case SDLK_y: return IK_Y;
		case SDLK_z: return IK_Z;

		// Digits and space.
		case SDLK_0: return IK_0;
		case SDLK_1: return IK_1;
		case SDLK_2: return IK_2;
		case SDLK_3: return IK_3;
		case SDLK_4: return IK_4;
		case SDLK_5: return IK_5;
		case SDLK_6: return IK_6;
		case SDLK_7: return IK_7;
		case SDLK_8: return IK_8;
		case SDLK_9: return IK_9;
		case SDLK_SPACE: return IK_Space;

		// TTY functions.
		case SDLK_BACKSPACE: return IK_Backspace;
		case SDLK_TAB:       return IK_Tab;
		case SDLK_RETURN:    return IK_Enter;
		case SDLK_PAUSE:     return IK_Pause;
		case SDLK_ESCAPE:    return IK_Escape;
		case SDLK_DELETE:    return IK_Delete;
		case SDLK_INSERT:    return IK_Insert;

		// Modifiers (frozen quirks: Ctrl collapse, GUI as F24).
		case SDLK_LSHIFT: return IK_LShift;
		case SDLK_RSHIFT: return IK_RShift;
		case SDLK_LCTRL:  return IK_Ctrl;
		case SDLK_RCTRL:  return IK_Ctrl;
		case SDLK_LGUI:   return IK_F24;
		case SDLK_RGUI:   return IK_F24;
		case SDLK_LALT:   return IK_Alt;
		case SDLK_RALT:   return IK_Alt;

		// Special remaps.
		case SDLK_BACKQUOTE:    return IK_Tilde;
		case SDLK_QUOTE:        return IK_SingleQuote;
		case SDLK_SEMICOLON:    return IK_Semicolon;
		case SDLK_COMMA:        return IK_Comma;
		case SDLK_PERIOD:       return IK_Period;
		case SDLK_SLASH:        return IK_Slash;
		case SDLK_BACKSLASH:    return IK_Backslash;
		case SDLK_LEFTBRACKET:  return IK_LeftBracket;
		case SDLK_RIGHTBRACKET: return IK_RightBracket;
		case 241:               return IK_Tilde; // Spanish N key
		case 167:               return IK_Tilde; // +/- section sign, Apple intl QWERTY

		// Function keys.
		case SDLK_F1:  return IK_F1;
		case SDLK_F2:  return IK_F2;
		case SDLK_F3:  return IK_F3;
		case SDLK_F4:  return IK_F4;
		case SDLK_F5:  return IK_F5;
		case SDLK_F6:  return IK_F6;
		case SDLK_F7:  return IK_F7;
		case SDLK_F8:  return IK_F8;
		case SDLK_F9:  return IK_F9;
		case SDLK_F10: return IK_F10;
		case SDLK_F11: return IK_F11;
		case SDLK_F12: return IK_F12;
		case SDLK_F13: return IK_F13;
		case SDLK_F14: return IK_F14;
		case SDLK_F15: return IK_F15;

		// Cursor control and motion.
		case SDLK_HOME:    return IK_Home;
		case SDLK_LEFT:    return IK_Left;
		case SDLK_UP:      return IK_Up;
		case SDLK_RIGHT:   return IK_Right;
		case SDLK_DOWN:    return IK_Down;
		case SDLK_PAGEUP:  return IK_PageUp;
		case SDLK_PAGEDOWN:return IK_PageDown;
		case SDLK_END:     return IK_End;

		// Keypad.
		case SDLK_KP_ENTER:   return IK_Enter;
		case SDLK_KP_0:       return IK_NumPad0;
		case SDLK_KP_1:       return IK_NumPad1;
		case SDLK_KP_2:       return IK_NumPad2;
		case SDLK_KP_3:       return IK_NumPad3;
		case SDLK_KP_4:       return IK_NumPad4;
		case SDLK_KP_5:       return IK_NumPad5;
		case SDLK_KP_6:       return IK_NumPad6;
		case SDLK_KP_7:       return IK_NumPad7;
		case SDLK_KP_8:       return IK_NumPad8;
		case SDLK_KP_9:       return IK_NumPad9;
		case SDLK_KP_MULTIPLY:return IK_GreyStar;
		case SDLK_KP_PLUS:    return IK_GreyPlus;
		case SDLK_KP_EQUALS:  return IK_Separator;
		case SDLK_KP_MINUS:   return IK_GreyMinus;
		case SDLK_KP_PERIOD:  return IK_NumPadPeriod;
		case SDLK_KP_DIVIDE:  return IK_GreySlash;

		// Other.
		case SDLK_MINUS:       return IK_Minus;
		case SDLK_EQUALS:      return IK_Equals;
		case SDLK_NUMLOCKCLEAR:return IK_NumLock;
		case SDLK_CAPSLOCK:    return IK_CapsLock;
		case SDLK_SCROLLLOCK:  return IK_ScrollLock;

		default: return IK_None;
	}
}

// SDL_KEYDOWN / SDL_KEYUP -> normalized key event. Unmapped keysyms keep
// Key=IK_None so the adopter can apply today's suppression rule centrally.
inline FHP2InputEvent HP2MakeKeyEvent(SDL_Keycode Sym, bool bPressed)
{
	FHP2InputEvent E = HP2MakeNoneEvent();
	E.Key = HP2MapKeysym(Sym);
	E.Action = bPressed ? IST_Press : IST_Release;
	return E;
}

// SDL_MOUSEBUTTONDOWN / SDL_MOUSEBUTTONUP -> normalized mouse-button event.
// Only buttons 1/2/3 reach the engine today; anything else maps to a
// suppressed IK_None event.
inline FHP2InputEvent HP2MakeMouseButtonEvent(Uint8 Button, bool bPressed)
{
	FHP2InputEvent E = HP2MakeNoneEvent();
	switch (Button)
	{
		case 1: E.Key = IK_LeftMouse;   break;
		case 2: E.Key = IK_MiddleMouse; break;
		case 3: E.Key = IK_RightMouse;  break;
		default: return E;              // suppressed: IK_None + IST_None
	}
	E.Action = bPressed ? IST_Press : IST_Release;
	return E;
}

// SDL_MOUSEWHEEL -> normalized wheel sequence, mirroring today's pump:
// one IK_MouseW axis event carrying the wheel delta, then a synthesized
// press/release pair on IK_MouseWheelUp or IK_MouseWheelDown. Returns the
// number of events written into Out (must hold at least 3); zero when the
// wheel delta is zero, matching the pump's `if (Event.wheel.y)` guard.
inline INT HP2MakeWheelEvents(Sint32 WheelY, FHP2InputEvent Out[3])
{
	if (WheelY == 0)
		return 0;

	FHP2InputEvent& Axis = Out[0];
	Axis = HP2MakeNoneEvent();
	Axis.Key = IK_MouseW; Axis.Action = IST_Axis; Axis.Delta = (FLOAT)WheelY;

	const EInputKey Direction = (WheelY < 0) ? IK_MouseWheelDown : IK_MouseWheelUp;

	FHP2InputEvent& Press = Out[1];
	Press = HP2MakeNoneEvent();
	Press.Key = Direction; Press.Action = IST_Press;

	FHP2InputEvent& Release = Out[2];
	Release = HP2MakeNoneEvent();
	Release.Key = Direction; Release.Action = IST_Release;

	return 3;
}

// First UTF-8 codepoint of a text payload; -1 when malformed. Mirrors
// ucs2_char_code_from_utf8() in SDLViewport.cpp (BMP-only UCS2 semantics,
// including its silent acceptance of truncated-but-prefixed sequences).
inline INT HP2Utf8Codepoint(const char* Utf8)
{
	if (Utf8 == NULL)
		return -1;
	const unsigned char C0 = (unsigned char)Utf8[0];
	if (C0 < 128)
		return C0;
	if (C0 < 224)
	{
		const unsigned char C1 = (unsigned char)Utf8[1];
		if (128 <= C1 && C1 < 192)
			return ((C0 & 31) << 6) | (C1 & 63);
		return -1;
	}
	if (C0 < 240)
	{
		const unsigned char C1 = (unsigned char)Utf8[1];
		if (128 <= C1 && C1 < 192)
		{
			const unsigned char C2 = (unsigned char)Utf8[2];
			if (128 <= C2 && C2 < 192)
				return ((C0 & 15) << 12) | ((C1 & 63) << 6) | (C2 & 63);
		}
		return -1;
	}
	return -1;
}

// SDL_TEXTINPUT -> normalized text event. The engine routes these through
// UEngine::Key (console KeyType), not InputEvent, so Action stays IST_None
// as the marker; Key carries the first decoded codepoint (IK_None when the
// payload is malformed). Text holds up to 7 payload bytes plus terminator.
inline FHP2InputEvent HP2MakeTextEvent(const char* Utf8Text)
{
	FHP2InputEvent E = HP2MakeNoneEvent();
	const INT Codepoint = HP2Utf8Codepoint(Utf8Text);
	E.Key = (Codepoint >= 0) ? (EInputKey)Codepoint : IK_None;
	if (Utf8Text != NULL)
	{
		INT Index = 0;
		for (; Index < 7 && Utf8Text[Index] != 0; ++Index)
			E.Text[Index] = Utf8Text[Index];
		E.Text[Index] = 0;
	}
	return E;
}

#endif //_INC_HP2_INPUT_EVENTS
