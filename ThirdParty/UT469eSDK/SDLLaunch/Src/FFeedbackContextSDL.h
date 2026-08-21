/*=============================================================================
	FFeedbackContextSDL.h: Unreal SDL user interface interaction.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Stijn Volckaert
=============================================================================*/

/*-----------------------------------------------------------------------------
	FFeedbackContextSDL.
-----------------------------------------------------------------------------*/

#include "FFeedbackContextAnsi.h"
#include <SDL2/SDL.h>

//
// Feedback context.
//
class FFeedbackContextSDL : public FFeedbackContextAnsi
{
	void Serialize( const TCHAR* V, EName Event )
	{
		guard(FFeedbackContextSDL::Serialize);
		if( Event==NAME_UserPrompt && (GIsClient || GIsEditor) )
			SDL_ShowSimpleMessageBox( SDL_MESSAGEBOX_ERROR, appToAnsi(LocalizeError("Warning",TEXT("Core"))), appToAnsi(V), NULL );
		else
			FFeedbackContextAnsi::Serialize( V, NAME_Warning );
		unguard;
	}
	UBOOL YesNof( const TCHAR* Fmt, ... )
	{
		va_list ArgPtr;
		va_start(ArgPtr, Fmt);
		FString Temp = FString::Printf(Fmt, ArgPtr);
		va_end(ArgPtr);

		guard(FFeedbackContextSDL::YesNof);
		if( GIsClient || GIsEditor )
		{
			const SDL_MessageBoxButtonData buttons[] = {
				{ /* .flags, .buttonid, .text */        0, 0, "no" },
				{ SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT, 1, "yes" }
			};
			const SDL_MessageBoxColorScheme colorScheme = {
				{ /* .colors (.r, .g, .b) */
					/* [SDL_MESSAGEBOX_COLOR_BACKGROUND] */
					{ 255,   0,   0 },
					/* [SDL_MESSAGEBOX_COLOR_TEXT] */
					{   0, 255,   0 },
					/* [SDL_MESSAGEBOX_COLOR_BUTTON_BORDER] */
					{ 255, 255,   0 },
					/* [SDL_MESSAGEBOX_COLOR_BUTTON_BACKGROUND] */
					{   0,   0, 255 },
					/* [SDL_MESSAGEBOX_COLOR_BUTTON_SELECTED] */
					{ 255,   0, 255 }
				}
			};
			const SDL_MessageBoxData messageboxdata = {
				SDL_MESSAGEBOX_INFORMATION, /* .flags */
				NULL, /* .window */
				appToAnsi(LocalizeError("Question",TEXT("Core"))), /* .title */
				appToAnsi(*Temp), /* .message */
				SDL_arraysize(buttons), /* .numbuttons */
				buttons, /* .buttons */
				&colorScheme /* .colorScheme */
			};
			int buttonid;
			if (SDL_ShowMessageBox(&messageboxdata, &buttonid) < 0) {
				SDL_Log("error displaying message box");
				return 0;
			}
			return buttonid == 1;
		}
		else
			return 0;
		unguard;
	}
};
