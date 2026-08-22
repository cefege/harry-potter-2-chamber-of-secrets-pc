/*=============================================================================
	SDLClient.cpp: USDLClient code.
	Copyright 2002 Epic Games, Inc. All Rights Reserved.

        SDL website: http://www.libsdl.org/

Revision history:
	* Created by Ryan C. Gordon, based on WinDrv.
      This is an updated rewrite of the original SDLDrv.
=============================================================================*/

#include "SDLDrv.h"

/*-----------------------------------------------------------------------------
	Class implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(USDLClient);

/*-----------------------------------------------------------------------------
	USDLClient implementation.
-----------------------------------------------------------------------------*/

SDL_Joystick *USDLClient::Joystick = NULL;
SDL_JoystickID USDLClient::JoystickInstanceId = -1;
int USDLClient::JoystickAxes = 0;
int USDLClient::JoystickButtons = 0;
int USDLClient::JoystickHats = 0;
int USDLClient::JoystickBalls = 0;


//
// USDLClient constructor.
//
USDLClient::USDLClient()
{
	guard(USDLClient::USDLClient);
	unguard;
}

//
// Static init.
//
void USDLClient::StaticConstructor()
{
	guard(USDLClient::StaticConstructor);

	new(GetClass(),TEXT("UseJoystick"),          RF_Public)UBoolProperty(CPP_PROPERTY(UseJoystick),          TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("JoystickNumber"),       RF_Public)UIntProperty(CPP_PROPERTY(JoystickNumber),        TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("JoystickHatNumber"),    RF_Public)UIntProperty(CPP_PROPERTY(JoystickHatNumber),     TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("StartupFullscreen"),    RF_Public)UBoolProperty(CPP_PROPERTY(StartupFullscreen),    TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("BorderlessWindow"),    RF_Public)UBoolProperty(CPP_PROPERTY(BorderlessWindow),     TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("UseDesktopResolution"),    RF_Public)UBoolProperty(CPP_PROPERTY(UseDesktopResolution), TEXT("Display"),  CPF_Config );	
	new(GetClass(),TEXT("UIScale"),             RF_Public)UFloatProperty(CPP_PROPERTY(UIScale),             TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("ScaleJBX"),             RF_Public)UFloatProperty(CPP_PROPERTY(ScaleJBX),            TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("ScaleJBY"),             RF_Public)UFloatProperty(CPP_PROPERTY(ScaleJBY),            TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("JoystickDeadZone"),    RF_Public)UFloatProperty(CPP_PROPERTY(JoystickDeadZone),    TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("IgnoreHat"),            RF_Public)UBoolProperty(CPP_PROPERTY(IgnoreHat),            TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("IgnoreUngrabbedMouse"), RF_Public)UBoolProperty(CPP_PROPERTY(IgnoreUngrabbedMouse), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("AllowUnicodeKeys"),     RF_Public)UBoolProperty(CPP_PROPERTY(AllowUnicodeKeys),     TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("AllowCommandQKeys"),    RF_Public)UBoolProperty(CPP_PROPERTY(AllowCommandQKeys),    TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("MacKeepAllScreensOn"),  RF_Public)UBoolProperty(CPP_PROPERTY(MacKeepAllScreensOn),  TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("TextToSpeechFile"),     RF_Public)UStrProperty(CPP_PROPERTY(TextToSpeechFile),      TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("MacNativeTextToSpeech"),RF_Public)UBoolProperty(CPP_PROPERTY(MacNativeTextToSpeech),TEXT("Display"),  CPF_Config );

	// Sensible default values
	WindowedViewportX=1024;
	WindowedViewportY=768;
	WindowedColorBits=32;
	FullscreenViewportX=1024;
	FullscreenViewportY=768;
	FullscreenColorBits=32;
	Brightness=0.5;
	MipFactor=1.0;
	CaptureMouse=true;
	StartupFullscreen=false;
	BorderlessWindow=false;
	UseDesktopResolution=false;
	UIScale=1.0f;
	JoystickDeadZone=0.2f;
	ScreenFlashes=true;
	MaintainVerticalFOV=true;
	Decals=true;
	MinDesiredFrameRate=30.0;
	ParticleDensity=1;

	unguard;
}

void USDLClient::CloseJoystick()
{
	SDL_Joystick* OldJoystick = Joystick;
	Joystick = NULL;
	JoystickInstanceId = -1;
	JoystickAxes = 0;
	JoystickButtons = 0;
	JoystickHats = 0;
	JoystickBalls = 0;

	if (OldJoystick != NULL)
		SDL_JoystickClose(OldJoystick);
}

UBOOL USDLClient::OpenJoystick()
{
	CloseJoystick();

	const INT JoystickCount = SDL_NumJoysticks();
	debugf(NAME_Init, TEXT("Detected %d joysticks"), JoystickCount);
	if (JoystickCount <= 0)
		return 0;

	if (JoystickNumber < 0 || JoystickNumber >= JoystickCount)
	{
		debugf(NAME_Init, TEXT("JoystickNumber exceeds the number of detected joysticks, setting to 0."));
		JoystickNumber = 0;
	}

	const ANSICHAR* JoyNameAnsi = SDL_JoystickNameForIndex(JoystickNumber);
	if (JoyNameAnsi == NULL)
		JoyNameAnsi = "Unknown Joystick";
	const TCHAR* JoyName = appFromAnsi(JoyNameAnsi);
	debugf(NAME_Init, TEXT("Joystick [%i] : %s"), JoystickNumber, JoyName);

	Joystick = SDL_JoystickOpen(JoystickNumber);
	if (Joystick == NULL)
	{
		debugf(NAME_Init, TEXT("Couldn't open joystick [%s]"), JoyName);
		return 0;
	}

	JoystickInstanceId = SDL_JoystickInstanceID(Joystick);
	if (JoystickInstanceId < 0)
	{
		debugf(NAME_Init, TEXT("Couldn't identify joystick [%s]"), JoyName);
		CloseJoystick();
		return 0;
	}

	JoystickButtons = Max(0, SDL_JoystickNumButtons(Joystick));
	debugf(NAME_Init, TEXT("Joystick has %i buttons"), JoystickButtons);

	JoystickHats = Max(0, SDL_JoystickNumHats(Joystick));
	debugf(NAME_Init, TEXT("Joystick has %i hats"), JoystickHats);

	JoystickBalls = Max(0, SDL_JoystickNumBalls(Joystick));
	debugf(NAME_Init, TEXT("Joystick has %i balls"), JoystickBalls);

	if (JoystickHatNumber < 0 || JoystickHatNumber >= JoystickHats)
	{
		debugf(NAME_Init, TEXT("JoystickHatNumber exceeds the number of detected hats, setting to 0."));
		JoystickHatNumber = 0;
	}

	if (JoystickButtons > 16)
		JoystickButtons = 16;

	if (JoystickButtons > 12 && JoystickHats > 0 && !IgnoreHat)
		JoystickButtons = 12;  /* joy13 is first hat "button" */

	if (JoystickButtons != SDL_JoystickNumButtons(Joystick))
	{
		debugf(NAME_Init, TEXT("Too many joystick buttons; clamped to %d."), JoystickButtons);
		if (JoystickHats > 0 && !IgnoreHat)
			debugf(NAME_Init, TEXT("(Disable hat switches with \"IgnoreHat=True\" to raise this.)"));
	}

	const INT DetectedAxes = Max(0, SDL_JoystickNumAxes(Joystick));
	JoystickAxes = Min(DetectedAxes, (INT)MaxJoystickAxes);
	debugf(NAME_Init, TEXT("Joystick has %i axes"), DetectedAxes);
	if (DetectedAxes > MaxJoystickAxes)
		debugf(NAME_Init, TEXT("Too many joystick axes; clamped to %d."), MaxJoystickAxes);

	return 1;
}

//
// Initialize the platform-specific viewport manager subsystem.
// Must be called after the Unreal object manager has been initialized.
// Must be called before any viewports are created.
//
void USDLClient::Init( UEngine* InEngine )
{
	guard(USDLClient::USDLClient);

#if UNREAL_TOURNAMENT_OLDUNREAL
	GClient = this;
#endif

	// Init base.
	UClient::Init( InEngine );

	// stijn: Make sure we capture the mouse as SDLDrv won't work well without it
	CaptureMouse = true;

	// Note configuration.
	PostEditChange();

	// Default res option.
	if( ParseParam(appCmdLine(),TEXT("defaultres")) )
	{
	    // gam ---
		WindowedViewportX  = FullscreenViewportX  /*= MenuViewportX*/   = 640;
		WindowedViewportY  = FullscreenViewportY  /*= MenuViewportY*/   = 480;
		// --- gam
	}

	// Joystick.
	if (!OpenJoystick() && SDL_NumJoysticks() > 0)
		UseJoystick = false;
	// Success.
	debugf( NAME_Init, TEXT("SDLClient initialized.") );

	unguard;
}


//
// Shut down the platform-specific viewport manager subsystem.
//
void USDLClient::Destroy()
{
	guard(USDLClient::Destroy);

	// Make sure to shut down Viewports first.
	for( INT i=0; i<Viewports.Num(); i++ )
		Viewports(i)->ConditionalDestroy();

	// Shut down GRenDev.
	if (GRenderDevice)
		GRenderDevice->Exit();

	CloseJoystick();

    SDL_Quit();

	debugf( NAME_Exit, TEXT("SDL client shut down") );
	Super::Destroy();

#if UNREAL_TOURNAMENT_OLDUNREAL
	GClient = NULL;
#endif
	unguard;
}

void USDLClient::TeardownSR()
{
	guard(USDLClient::TeardownSR);
    // no-op, currently.
    unguard;
}

//
// Failsafe routine to shut down viewport manager subsystem
// after an error has occured. Not guarded.
//
void USDLClient::ShutdownAfterError()
{
	debugf( NAME_Exit, TEXT("Executing USDLClient::ShutdownAfterError") );
	CloseJoystick();
    SDL_Quit();

	if (Engine && Engine->Audio)
	{
		Engine->Audio->ConditionalShutdownAfterError();
	}

	for (INT i = Viewports.Num() - 1; i >= 0; i--)
	{
		USDLViewport *Viewport = (USDLViewport *) Viewports(i);
		Viewport->ConditionalShutdownAfterError();
	}

	Super::ShutdownAfterError();
}

void USDLClient::NotifyDestroy( void* Src )
{
	guard(USDLClient::NotifyDestroy);
	unguard;
}

//
// Command line.
//
UBOOL USDLClient::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(USDLClient::Exec);
	if( UClient::Exec( Cmd, Ar ) )
	{
		return 1;
	}
	return 0;
	unguard;
}

//
// Perform timer-tick processing on all visible viewports.  This causes
// all realtime viewports, and all non-realtime viewports which have been
// updated, to be blitted.
//
void USDLClient::Tick()
{
	guard(USDLClient::Tick);
	
	// Blit any viewports that need blitting.
  	for( INT i=0; i<Viewports.Num(); i++ )
	{
		USDLViewport* Viewport = CastChecked<USDLViewport>(Viewports(i));
		//check(!Viewport->HoldCount);
		/*		
		//dv: FIXME???
		if( !(Viewport->XWindow) )
		{
			// Window was closed via close button.
			delete Viewport;
			return;
		}
		*/
  		//else
		SDL_Window* Window = static_cast<SDL_Window*>(Viewport->GetWindow());
		const Uint32 WindowFlags = Window ? SDL_GetWindowFlags(Window) : 0;
		if
		(   Viewport->IsRealtime()
		&&	Viewport->SizeX
		&&	Viewport->SizeY
		&&	(WindowFlags & (SDL_WINDOW_MINIMIZED | SDL_WINDOW_HIDDEN)) == 0 )
		{
			Viewport->Repaint(1);
		}
	}
	unguard;
}

//
// Create a new viewport.
//
UViewport* USDLClient::NewViewport( const FName Name )
{
	guard(USDLClient::NewViewport);
	return new( this, Name )USDLViewport();
	unguard;
}

//
// Configuration change.
//
void USDLClient::PostEditChange()
{
	guard(USDLClient::PostEditChange);
	if( appIsNan(UIScale) )
		UIScale = 1.0f;
	else
		UIScale = Clamp(UIScale, 0.75f, 2.0f);
	if( appIsNan(JoystickDeadZone) )
		JoystickDeadZone = 0.2f;
	else
		JoystickDeadZone = Clamp(JoystickDeadZone, 0.0f, 0.99f);
	Super::PostEditChange();
	unguard;
}

//
// Enable or disable all viewport windows that have ShowFlags set (or all if ShowFlags=0).
//
void USDLClient::EnableViewportWindows( DWORD ShowFlags, int DoEnable )
{
	guard(USDLClient::EnableViewportWindows);
    /* meaningless in terms of SDL. */
	unguard;
}

//
// Show or hide all viewport windows that have ShowFlags set (or all if ShowFlags=0).
//
void USDLClient::ShowViewportWindows( DWORD ShowFlags, int DoShow )
{
	guard(USDLClient::ShowViewportWindows); 	
    /* meaningless in terms of SDL. */
	unguard;
}

//
// Make this viewport the current one.
// If Viewport=0, makes no viewport the current one.
//
void USDLClient::MakeCurrent( UViewport* InViewport )
{
	guard(USDLClient::MakeCurrent);
	for( INT i=0; i<Viewports.Num(); i++ )
	{
		UViewport* OldViewport = Viewports(i);
		if( OldViewport->Current && OldViewport!=InViewport )
		{
			OldViewport->Current = 0;
			OldViewport->UpdateWindowFrame();
		}
	}
	if( InViewport )
	{
//		LastCurrent = InViewport;
		InViewport->Current = 1;
		InViewport->UpdateWindowFrame();
	}
	unguard;
}

//
// Copy text to clipboard.
//
UBOOL USDLClient::SetClipboardText( FString& Str )
{
	guard(USDLClient::SetClipboardText);
	TArray<ANSICHAR> Utf8Buffer( 4 * Str.Len() + 1 );
	if( !appToUtf8InPlace(&Utf8Buffer(0), *Str, Utf8Buffer.Num()) )
		return 0;
	return SDL_SetClipboardText(&Utf8Buffer(0)) == 0;
	unguard;
}

//
// Paste text from clipboard.
//
FString USDLClient::GetClipboardText()
{
	guard(USDLClient::GetClipboardText);

	ANSICHAR* Utf8Buffer = SDL_GetClipboardText();
	if( !Utf8Buffer )
		return FString();
	const size_t Utf8Length = strlen(Utf8Buffer) + 1;
	if( Utf8Length>static_cast<size_t>(MAXINT) )
	{
		SDL_free(Utf8Buffer);
		return FString();
	}
	FString Result;
	TArray<TCHAR>& ResultBuffer = Result.GetCharArray();
	ResultBuffer.Add(static_cast<INT>(Utf8Length));
	if( !appFromUtf8InPlace(&ResultBuffer(0), Utf8Buffer, ResultBuffer.Num()) )
		Result.Empty();
	SDL_free(Utf8Buffer);
	
	return Result;
	unguard;
}

//
// Start/StopTextInput - We don't need these for SDL2
//
void USDLClient::StartTextInput()
{
}

void USDLClient::StopTextInput()
{
}

//
// DPI awareness - completely untested
//
INT USDLClient::GetDPIScaledX(INT X)
{
	const FLOAT DefaultDPI =
#if MACOSX
		72.f;
#else
		96.f;
#endif

	FLOAT DesktopDPI;
	if (SDL_GetDisplayDPI(0, NULL, &DesktopDPI, NULL) == 0)
	{
		return appCeil(static_cast<FLOAT>(X) * (DesktopDPI / DefaultDPI));
	}
	
	return X;
}

INT USDLClient::GetDPIScaledY(INT Y)
{
	const FLOAT DefaultDPI =
#if MACOSX
		72.f;
#else
		96.f;
#endif

	FLOAT DesktopDPI;
	if (SDL_GetDisplayDPI(0, NULL, &DesktopDPI, NULL) == 0)
	{
		return appCeil(static_cast<FLOAT>(Y) * (DesktopDPI / DefaultDPI));
	}
	
	return Y;
}

#if UNREAL_TOURNAMENT_OLDUNREAL
#include <SDL2/SDL_ttf.h>

//
// CreateTTFFont 
//
void* USDLClient::CreateTTFFont
(
	const TCHAR* FontName,
	int Height,
	int Italic,
	int Bold,
	int Underlined,
	int AntiAliased
)
{
	// Initialize the SDL TTF extension
	if (!TTF_WasInit() && TTF_Init())
	{
		debugf(TEXT("Couldn't initialize SDL TTF"));
		return nullptr;
	}

	// Load the TTF file
	debugf(TEXT("Looking for font %s"), FontName);

	// Remap the name - we currently use OpenSans as the alternative to Arial and Tahoma
	FString TTFFileName = FString::Printf(TEXT("Fonts/%s.ttf"), FontName);
	if (GFileManager->FileSize(*TTFFileName) <= 0)
	{
		if (!appStricmp(FontName, TEXT("Times")))
			TTFFileName = TEXT("Fonts/Tinos-Regular.ttf");
		else if (!appStricmp(FontName, TEXT("Courier")))
			TTFFileName = TEXT("Fonts/CourierPrime.ttf");
		else
			TTFFileName = TEXT("Fonts/OpenSans-Regular.ttf");
	}

	TTF_Font* InFont = TTF_OpenFont(appToAnsi(*TTFFileName), Height);

	if (!InFont)
	{
		debugf(TEXT("Couldn't load requested font: %s (file: %s)"), FontName, *TTFFileName);
		return nullptr;
	}

	debugf(TEXT("Opened font: %s"), *TTFFileName);

	// Apply the requested style
	if (Italic || Bold || Underlined)
	{
		TTF_SetFontStyle(InFont,
						 (Italic ? TTF_STYLE_ITALIC : 0) |
						 (Bold ? TTF_STYLE_BOLD : 0) |
						 (Underlined ? TTF_STYLE_UNDERLINE : 0));
	}

	return InFont;
}

//
// Checks which of the requested glyphs are available. Then renders then and stores their metrics
//
void USDLClient::MakeGlyphsList(FBitmask& RequestedGlyphs, void* InFont, TArray<FGlyphInfo>& OutGlyphs, INT AntiAlias)
{
	TCHAR Ch = 0;
	SDL_Color White = {255, 255, 255, 255};
	SDL_Color Black = {0, 0, 0, 255};

	TTF_Font* Font = reinterpret_cast<TTF_Font*>(InFont);
	if (!Font)
		return;

	for (; Ch < 65536 && Ch < RequestedGlyphs.HighestBitSet; ++Ch)
	{
		if (RequestedGlyphs.GetBit(Ch) && TTF_GlyphIsProvided(Font, Ch))
		{
			INT MinX, MaxX, MinY, MaxY, Advance, W, H;
			if (TTF_GlyphMetrics(Font, Ch, &MinX, &MaxX, &MinY, &MaxY, &Advance) == 0)
			{
				SDL_Surface* GlyphSurf = nullptr;
				if (MaxX - MinX > 0 && MaxY - MinY > 0)
				{
					// Do we really want to support non-AA rendering? It looks terrible
					if (AntiAlias)
						GlyphSurf = TTF_RenderGlyph_Shaded(Font, Ch, White, Black);
					else
						GlyphSurf = TTF_RenderGlyph_Solid(Font, Ch, White);
				}

				// stijn: SDL_ttf/FreeType glyph boxes have an empty line at the top
				INT BlitHeight   = Max(GlyphSurf ? GlyphSurf->h - 1 : 0, 0);
				INT BlitWidth    = GlyphSurf ? GlyphSurf->w : Advance;

				FGlyphInfo Info;
				Info.Ch = Ch;
				Info.BoxHeight = Info.CellHeight = BlitHeight;
				Info.BoxWidth = Info.CellWidth = BlitWidth;
				Info.GlyphSurf = GlyphSurf;
				OutGlyphs.AddItem(Info);
			}
		}
	}

	debugf(TEXT("Collected metrics for %d glyphs"), OutGlyphs.Num());
}

//
// The SDL version of this function just copies over existing SDL_Surface bits to the target PageTexture
//
void USDLClient::RenderGlyph(FGlyphInfo& Info, UTexture* PageTexture, INT X, INT Y, INT YAdjust)
{
	// Blit into the PageInfo texture
	SDL_Surface* GlyphSurf = reinterpret_cast<SDL_Surface*>(Info.GlyphSurf);

	// stijn: compensate for the line we removed when calculating BlitHeight
	YAdjust -= 1;
	
	if (Info.CellHeight > 0 && GlyphSurf)
	{
		for (INT h = 0; h < Info.CellHeight; ++h)
		{
			BYTE* PixelPtr = reinterpret_cast<BYTE*>(
				reinterpret_cast<PTRINT>(GlyphSurf->pixels) +
				(h - YAdjust) * GlyphSurf->pitch);
			
			for (INT w = 0; w < Info.CellWidth; ++w)
			{
				INT MipsOffset    = (PageTexture->USize * (Y + h) + X + w);

				if ((YAdjust > 0 && h < YAdjust) ||
					(YAdjust < 0 && h >= Info.CellHeight - Abs(YAdjust)))
				{
					PageTexture->Mips(0).DataArray(MipsOffset) = 1;
				}
				else
				{
					BYTE PaletteIndex = *PixelPtr++;
					SDL_Color* Color  = &GlyphSurf->format->palette->colors[PaletteIndex];

					// stijn: tried to create BGRA fonts but the engine doesn't like them
					// INT MipsOffset    = (PageTexture->USize * Y + X) * 4;
					// PageTexture->Mips(0).DataArray(MipsOffset + 3) = Color->a;
					// PageTexture->Mips(0).DataArray(MipsOffset + 2) = Color->r;
					// PageTexture->Mips(0).DataArray(MipsOffset + 1) = Color->g;
					// PageTexture->Mips(0).DataArray(MipsOffset    ) = Color->b;
					
					PageTexture->Mips(0).DataArray(MipsOffset    ) = Color->r;
				}
			}
		}
	}
}

//
// Frees SDL_Surfaces
//
void USDLClient::DestroyGlyphsList(TArray<FGlyphInfo>& Glyphs)
{
	for (INT i = 0; i < Glyphs.Num(); ++i)
	{
		FGlyphInfo& Info = Glyphs(i);
		if (Info.GlyphSurf)
			SDL_FreeSurface(reinterpret_cast<SDL_Surface*>(Info.GlyphSurf));
	}
}

//
// Frees the TTF_Font
//
void USDLClient::DestroyTTFFont(void* Font)
{
	if (Font)
		TTF_CloseFont(reinterpret_cast<TTF_Font*>(Font));
}
#endif

/*-----------------------------------------------------------------------------
    That's all, folks.
-----------------------------------------------------------------------------*/

