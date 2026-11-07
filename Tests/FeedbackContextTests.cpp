#define SDL_MAIN_HANDLED
#include <SDL2/SDL.h>

#include <cstdio>
#include <cstring>
#include <string>

#include "Core.h"
#include "FMallocAnsi.h"

// Replace only the SDL boundary: exercise the shipping feedback context and
// UTF-8 conversion without opening a modal dialog during automated tests.
static int TestShowMessageBox( const SDL_MessageBoxData* Box, int* Button );
static int TestShowSimpleMessageBox( Uint32, const char*, const char*, SDL_Window* );
#define SDL_ShowMessageBox TestShowMessageBox
#define SDL_ShowSimpleMessageBox TestShowSimpleMessageBox
#include "../HarryPotter2/Unreal/SDLLaunch/Src/FFeedbackContextSDL.h"
#undef SDL_ShowMessageBox
#undef SDL_ShowSimpleMessageBox

extern "C" { TCHAR GPackage[64] = TEXT("FeedbackContextTests"); }

static FMallocAnsi GTestAllocator;
static int GCalls = 0;
static int GResponse = 1;
static int GStatus = 0;
static bool GMessageMatches = false;
static bool GButtonsMatch = false;
static const char* GExpected = NULL;

static int TestShowMessageBox( const SDL_MessageBoxData* Box, int* Button )
{
	++GCalls;
	// Exercise heap activity before consuming the pointer, as an SDL backend
	// may do. ASan can additionally detect a pointer to a destroyed string.
	std::string Scratch( std::strlen(GExpected), 'x' );
	GMessageMatches = Box->message && std::strcmp( Box->message, GExpected ) == 0
		&& Scratch.size() == std::strlen(GExpected);
	GButtonsMatch = std::strcmp( Box->title, "Confirmation" ) == 0
		&& Box->numbuttons == 2 && Box->buttons[0].buttonid == 1 && Box->buttons[1].buttonid == 0;
	*Button = GResponse;
	return GStatus;
}

static int TestShowSimpleMessageBox( Uint32, const char*, const char*, SDL_Window* )
{
	return 0;
}

int main()
{
	GMalloc = &GTestAllocator;
	GIsClient = 1;
	FFeedbackContextSDL Feedback;
	GExpected = "A confirmation message long enough to require heap storage: "
		"the complete text must remain valid until the message box returns.";
	const TCHAR* Message = TEXT("A confirmation message long enough to require heap storage: "
		"the complete text must remain valid until the message box returns.");
	if( !Feedback.YesNof( Message ) || GCalls != 1 || !GMessageMatches || !GButtonsMatch )
	{
		std::fprintf( stderr, "confirmation message or Yes response failed\n" );
		return 1;
	}
	GResponse = 0;
	if( Feedback.YesNof( Message ) || GCalls != 2 || !GMessageMatches )
	{
		std::fprintf( stderr, "confirmation No response failed\n" );
		return 1;
	}
	GStatus = -1;
	if( !Feedback.YesNof( Message ) || GCalls != 3 || !GMessageMatches )
	{
		std::fprintf( stderr, "confirmation error default failed\n" );
		return 1;
	}
	GStatus = 0;
	GResponse = 1;
	GExpected = "Confirm caf\xc3\xa9?";
	if( !Feedback.YesNof( TEXT("Confirm caf\u00e9?") ) || GCalls != 4 || !GMessageMatches )
	{
		std::fprintf( stderr, "UTF-8 confirmation message failed\n" );
		return 1;
	}
	GIsClient = 0;
	GIsEditor = 0;
	if( !Feedback.YesNof( Message ) || GCalls != 4 )
	{
		std::fprintf( stderr, "non-client confirmation default failed\n" );
		return 1;
	}
	return 0;
}
