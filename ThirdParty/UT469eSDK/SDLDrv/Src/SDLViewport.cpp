/*=============================================================================
	SDLViewport.cpp: USDLViewport code.
	Copyright 2002 Epic Games, Inc. All Rights Reserved.

        SDL website: http://www.libsdl.org/

Revision history:
	* Created by Ryan C. Gordon, based on WinDrv.
      This is an updated rewrite of the original SDLDrv.
=============================================================================*/
 
#include <stdlib.h>
#include <unistd.h>
#include <errno.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <ctype.h>
#include <dlfcn.h>
#include "SDLDrv.h"
#include "FConfigCacheIni.h"

// Duplicated here for convenience.
#if UNREAL_TOURNAMENT_OLDUNREAL
enum EEditorMode
{
	EM_None 				= 0,	// Gameplay, editor disabled.
	EM_ViewportMove			= 1,	// Move viewport normally.
	EM_ViewportZoom			= 2,	// Move viewport with acceleration.
	EM_ActorRotate			= 5,	// Rotate actor.
	EM_ActorScale			= 8,	// Scale actor.
	EM_TexturePan			= 11,	// Pan textures.
	EM_TextureRotate		= 13,	// Rotate textures.
	EM_TextureScale			= 14,	// Scale textures.
	EM_ActorSnapScale		= 18,	// Actor snap-scale.
	EM_TexView				= 19,	// Viewing textures.
	EM_TexBrowser			= 20,	// Browsing textures.
	EM_StaticMeshBrowser	= 21,	// Browsing static meshes.
	EM_MeshView				= 22,	// Viewing mesh.
	EM_MeshBrowser			= 23,	// Browsing mesh.
	EM_BrushClip			= 24,	// Brush Clipping.
	EM_VertexEdit			= 25,	// Multiple Vertex Editing.
	EM_FaceDrag				= 26,	// Face Dragging.
	EM_Polygon				= 27,	// Free hand polygon drawing
	EM_TerrainEdit			= 28,	// Terrain editing.
	EM_PrefabBrowser		= 29,	// Browsing prefabs.
	EM_Matinee				= 30,	// Movie editing.
	EM_EyeDropper			= 31,	// Eyedropper
	EM_Animation			= 32,	// Viewing Animation
	EM_FindActor			= 33,	// Find Actor
	EM_MaterialEditor		= 34,	// Material editor
	EM_Geometry				= 35,	// Geometry editing mode
	EM_NewCameraMove		= 50,
};
#endif

/*-----------------------------------------------------------------------------
	Class implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(USDLViewport);

/*-----------------------------------------------------------------------------
	USDLViewport Init/Exit.
-----------------------------------------------------------------------------*/

static UBOOL FullscreenOnly = 0;   // !!! FIXME: UE2 compat hack.
static SDL_Cursor* StandardCursors[7] = {NULL};
static const SDL_Cursor* ActiveCursor = NULL;
static UBOOL InResize = 0;
static void* HostPointerFromLegacyHandle(DWORD Handle)
{
	static_assert(sizeof(DWORD) <= sizeof(UPTRINT), "legacy window handle must widen safely");
	static_assert(sizeof(void*) == sizeof(UPTRINT), "host pointer bridge width mismatch");
	const UPTRINT HostValue = static_cast<UPTRINT>(Handle);
	void* Result = NULL;
	appMemcpy(&Result, &HostValue, sizeof(Result));
	return Result;
}


//
// Constructor.
//
USDLViewport::USDLViewport()
:	UViewport()
{
	guard(USDLViewport::USDLViewport);

    FullscreenOnly = 0;

    LostFullscreen = LostGrab = 0;

    TextToSpeechObject = -1;

    #if MACOSX
    MacTextToSpeechEnabled = 0;
    #endif

    LastUpdateTime = appSeconds();

	// Query current bit depth.
	SDL_DisplayMode mode;
	if (SDL_GetDesktopDisplayMode(0, &mode) == -1)
	{
		mode.format = SDL_PIXELFORMAT_BGRA8888;
	}

	int depth = (int) (SDL_BITSPERPIXEL(mode.format));
	if (depth > 32)
		depth = 32;
	switch (depth)
	{
		case 8 :	
			ColorBytes = 2;
			break;

		case 15 :	
			ColorBytes = 2;		
			break;

		case 16 :
			ColorBytes = 2;
			Caps |= CC_RGB565;
			break;

		case 24 :
		case 32 :
			ColorBytes = 4;
			break;

		default :	
			ColorBytes = 2;
			Caps |= CC_RGB565;
	}

	// Remap important keys.

	KeysymMap.Set(SDLK_a, IK_A);
	KeysymMap.Set(SDLK_b, IK_B);
	KeysymMap.Set(SDLK_c, IK_C);
	KeysymMap.Set(SDLK_d, IK_D);
	KeysymMap.Set(SDLK_e, IK_E);
	KeysymMap.Set(SDLK_f, IK_F);
	KeysymMap.Set(SDLK_g, IK_G);
	KeysymMap.Set(SDLK_h, IK_H);
	KeysymMap.Set(SDLK_i, IK_I);
	KeysymMap.Set(SDLK_j, IK_J);
	KeysymMap.Set(SDLK_k, IK_K);
	KeysymMap.Set(SDLK_l, IK_L);
	KeysymMap.Set(SDLK_m, IK_M);
	KeysymMap.Set(SDLK_n, IK_N);
	KeysymMap.Set(SDLK_o, IK_O);
	KeysymMap.Set(SDLK_p, IK_P);
	KeysymMap.Set(SDLK_q, IK_Q);
	KeysymMap.Set(SDLK_r, IK_R);
	KeysymMap.Set(SDLK_s, IK_S);
	KeysymMap.Set(SDLK_t, IK_T);
	KeysymMap.Set(SDLK_u, IK_U);
	KeysymMap.Set(SDLK_v, IK_V);
	KeysymMap.Set(SDLK_w, IK_W);
	KeysymMap.Set(SDLK_x, IK_X);
	KeysymMap.Set(SDLK_y, IK_Y);
	KeysymMap.Set(SDLK_z, IK_Z);
	KeysymMap.Set(SDLK_0, IK_0);
	KeysymMap.Set(SDLK_1, IK_1);
	KeysymMap.Set(SDLK_2, IK_2);
	KeysymMap.Set(SDLK_3, IK_3);
	KeysymMap.Set(SDLK_4, IK_4);
	KeysymMap.Set(SDLK_5, IK_5);
	KeysymMap.Set(SDLK_6, IK_6);
	KeysymMap.Set(SDLK_7, IK_7);
	KeysymMap.Set(SDLK_8, IK_8);
	KeysymMap.Set(SDLK_9, IK_9);
	KeysymMap.Set(SDLK_SPACE, IK_Space);

	// TTY Functions.
	KeysymMap.Set(SDLK_BACKSPACE, IK_Backspace);
	KeysymMap.Set(SDLK_TAB, IK_Tab);
	KeysymMap.Set(SDLK_RETURN, IK_Enter);
	KeysymMap.Set(SDLK_PAUSE, IK_Pause);
	KeysymMap.Set(SDLK_ESCAPE, IK_Escape);
	KeysymMap.Set(SDLK_DELETE, IK_Delete);
	KeysymMap.Set(SDLK_INSERT, IK_Insert);

	// Modifiers.
	KeysymMap.Set(SDLK_LSHIFT, IK_LShift);
	KeysymMap.Set(SDLK_RSHIFT, IK_RShift);
//	KeysymMap.Set(SDLK_LCTRL, IK_LControl);
//	KeysymMap.Set(SDLK_RCTRL, IK_RControl);
	KeysymMap.Set(SDLK_LCTRL, IK_Ctrl);
	KeysymMap.Set(SDLK_RCTRL, IK_Ctrl);
	KeysymMap.Set(SDLK_LGUI, IK_F24);
	KeysymMap.Set(SDLK_RGUI, IK_F24);
	KeysymMap.Set(SDLK_LALT, IK_Alt);
	KeysymMap.Set(SDLK_RALT, IK_Alt);
	
	// Special remaps.
	KeysymMap.Set(SDLK_BACKQUOTE, IK_Tilde);
	KeysymMap.Set(SDLK_QUOTE, IK_SingleQuote);
	KeysymMap.Set(SDLK_SEMICOLON, IK_Semicolon);
	KeysymMap.Set(SDLK_COMMA, IK_Comma);
	KeysymMap.Set(SDLK_PERIOD, IK_Period);
	KeysymMap.Set(SDLK_SLASH, IK_Slash);
	KeysymMap.Set(SDLK_BACKSLASH, IK_Backslash);
	KeysymMap.Set(SDLK_LEFTBRACKET, IK_LeftBracket);
	KeysymMap.Set(SDLK_RIGHTBRACKET, IK_RightBracket);
	KeysymMap.Set(241, IK_Tilde); // Spanish Ñ key
	KeysymMap.Set(167, IK_Tilde); // ±/§ key on Apple international QWERTY layout 
 
	// Misc function keys.
	KeysymMap.Set(SDLK_F1, IK_F1);
	KeysymMap.Set(SDLK_F2, IK_F2);
	KeysymMap.Set(SDLK_F3, IK_F3);
	KeysymMap.Set(SDLK_F4, IK_F4);
	KeysymMap.Set(SDLK_F5, IK_F5);
	KeysymMap.Set(SDLK_F6, IK_F6);
	KeysymMap.Set(SDLK_F7, IK_F7);
	KeysymMap.Set(SDLK_F8, IK_F8);
	KeysymMap.Set(SDLK_F9, IK_F9);
	KeysymMap.Set(SDLK_F10, IK_F10);
	KeysymMap.Set(SDLK_F11, IK_F11);
	KeysymMap.Set(SDLK_F12, IK_F12);
	KeysymMap.Set(SDLK_F13, IK_F13);
	KeysymMap.Set(SDLK_F14, IK_F14);
	KeysymMap.Set(SDLK_F15, IK_F15);

	// Cursor control and motion.
	KeysymMap.Set(SDLK_HOME, IK_Home);
	KeysymMap.Set(SDLK_LEFT, IK_Left);
	KeysymMap.Set(SDLK_UP, IK_Up);
	KeysymMap.Set(SDLK_RIGHT, IK_Right);
	KeysymMap.Set(SDLK_DOWN, IK_Down);
	KeysymMap.Set(SDLK_PAGEUP, IK_PageUp);
	KeysymMap.Set(SDLK_PAGEDOWN, IK_PageDown);
	KeysymMap.Set(SDLK_END, IK_End);

	// Keypad functions and numbers.
	KeysymMap.Set(SDLK_KP_ENTER, IK_Enter);
	KeysymMap.Set(SDLK_KP_0, IK_NumPad0);
	KeysymMap.Set(SDLK_KP_1, IK_NumPad1);
	KeysymMap.Set(SDLK_KP_2, IK_NumPad2);
	KeysymMap.Set(SDLK_KP_3, IK_NumPad3);
	KeysymMap.Set(SDLK_KP_4, IK_NumPad4);
	KeysymMap.Set(SDLK_KP_5, IK_NumPad5);
	KeysymMap.Set(SDLK_KP_6, IK_NumPad6);
	KeysymMap.Set(SDLK_KP_7, IK_NumPad7);
	KeysymMap.Set(SDLK_KP_8, IK_NumPad8);
	KeysymMap.Set(SDLK_KP_9, IK_NumPad9);
	KeysymMap.Set(SDLK_KP_MULTIPLY, IK_GreyStar);
	KeysymMap.Set(SDLK_KP_PLUS, IK_GreyPlus);
	KeysymMap.Set(SDLK_KP_EQUALS, IK_Separator);
	KeysymMap.Set(SDLK_KP_MINUS, IK_GreyMinus);
	KeysymMap.Set(SDLK_KP_PERIOD, IK_NumPadPeriod);
	KeysymMap.Set(SDLK_KP_DIVIDE, IK_GreySlash);

	// Other
	KeysymMap.Set(SDLK_MINUS, IK_Minus);
	KeysymMap.Set(SDLK_EQUALS, IK_Equals);
	KeysymMap.Set(SDLK_NUMLOCKCLEAR, IK_NumLock);
	KeysymMap.Set(SDLK_CAPSLOCK, IK_CapsLock);
	KeysymMap.Set(SDLK_SCROLLLOCK, IK_ScrollLock);

	LastJoyHat = IK_None;
	StandardCursors[0] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_ARROW);
	StandardCursors[1] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_SIZEALL);
	StandardCursors[2] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_SIZENESW);
	StandardCursors[3] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_SIZENS);
	StandardCursors[4] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_SIZENWSE);
	StandardCursors[5] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_SIZEWE);
	StandardCursors[6] = SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_WAIT);

	if (StandardCursors[0])
	{
		ActiveCursor = StandardCursors[0];
		SDL_SetCursor(StandardCursors[0]);
		bWindowsMouseAvailable=1;
	}
	debugf( TEXT("Created and initialized a new SDL viewport.") );

	SDL_version compiled, linked;
	SDL_VERSION(&compiled);
	SDL_GetVersion(&linked);
	debugf(TEXT("Compiled with SDL %d.%d.%d, linked with %d.%d.%d"),
		   compiled.major, compiled.minor, compiled.patch,
		   linked.major, linked.minor, linked.patch);
	unguard;
}
UFont* USDLViewport::CreateNativeFont( const TCHAR* FontName, int Height )
{
	if( !FontName || !*FontName || Height<=0 )
		return NULL;

	UFont* Font = new UFont;
	Font->FontName = FontName;
	Font->FontHeight = Height;
	Font->NativeFont = NULL;
	return Font;
}

void USDLViewport::DrawString( DWORD Flags, UFont* Font, INT& DrawX, INT& DrawY, const TCHAR* Text, const FPlane& Color )
{
	if( !RenDev || !Canvas || !Canvas->Frame || !GetOuterUClient() || !Font || !Text || !*Text )
		return;

	const INT SourceLength = appStrlen(Text);
	FCanvasTextRequest Request = {};
	Request.Font = Font;
	Request.Text = Text;
	Request.TextLength = -SourceLength;
	Request.TextScale = 1.f;
	Request.OriginX = 0;
	Request.OriginY = 0;
	Request.ClipX = SizeX;
	Request.ClipY = SizeY;
	Request.StartX = DrawX;
	Request.StartY = DrawY;
	Request.PolyFlags = Flags;
	Request.Color = Color;
	Request.VisibleSourceCharacters = SourceLength;
	Request.bCenter = (Flags & PF_TwoSided) != 0;

	FCanvasTextLayout* Layout = NULL;
	INT Width = 0, Height = 0;
	if( RenDev->CreateCanvasTextLayout(Request, Layout) &&
		RenDev->MeasureCanvasText(Layout, Width, Height) &&
		RenDev->DrawCanvasText(Canvas->Frame, Layout) )
	{
		DrawX += Width;
		DrawY += Height;
	}
	if( Layout )
		RenDev->DestroyCanvasTextLayout(Layout);
}

//
// Destroy.
//
void USDLViewport::Destroy()
{
	guard(USDLViewport::Destroy);
	Super::Destroy();

	if( BlitFlags & BLIT_Temporary )
		appFree( ScreenPointer );

    if (TextToSpeechObject != -1)
    {
        close(TextToSpeechObject);
        TextToSpeechObject = -1;
    }

    #if MACOSX
    if (MacTextToSpeechEnabled)
    {
        StopSpeech(MacTextToSpeechChannel);
        DisposeSpeechChannel(MacTextToSpeechChannel);
        SpeechQueue.Empty();
        MacTextToSpeechEnabled = 0;
    }
    #endif

	for (int i = 0; i < ARRAY_COUNT(StandardCursors); ++i)
		if (StandardCursors[i])
			SDL_FreeCursor(StandardCursors[i]);

	unguard;
}

//
// Error shutdown.
//
void USDLViewport::ShutdownAfterError()
{
	SDL_Quit();
	Super::ShutdownAfterError();
}

// where did we enter/leave the non-grabbed window
INT MouseLeaveX = 0, MouseEnterX = 0, MouseLeaveY = 0, MouseEnterY = 0;

void USDLViewport::GetMouseState(INT* MouseX, INT* MouseY)
{
	if (MouseIsGrabbed)
	{
		SDL_GetRelativeMouseState(MouseX, MouseY);
	}
	else
	{
		SDL_GetMouseState(MouseX, MouseY);
	}
}

void USDLViewport::UpdateMouseGrabState(const UBOOL bGrab)
{
	bool OldMouseIsGrabbed = MouseIsGrabbed;
	MouseIsGrabbed = bGrab;

	if (OldMouseIsGrabbed != MouseIsGrabbed)
	{
		// going back to not grabbed
		if (!MouseIsGrabbed)
		{
			GetMouseState(&MouseEnterX, &MouseEnterY);
			if (ActiveCursor)
				SDL_ShowCursor(SDL_ENABLE);
			WindowsMouseX = Clamp<FLOAT>(MouseEnterX * MouseScaleX, 0.f, SizeX);
			WindowsMouseY = Clamp<FLOAT>(MouseEnterY * MouseScaleY, 0.f, SizeY);
		}
		// going to grabbed
		else
		{
			GetMouseState(&MouseLeaveX, &MouseLeaveY);
			if (ActiveCursor)
				SDL_ShowCursor(SDL_DISABLE);
		}
	}

#ifndef __EMSCRIPTEN__  // we grab this elsewhere for Emscripten.
	SDL_SetHint(SDL_HINT_MOUSE_RELATIVE_MODE_WARP, "0");
	SDL_SetHint(SDL_HINT_MOUSE_RELATIVE_SCALING, "0");
	SDL_SetRelativeMouseMode(MouseIsGrabbed ? SDL_TRUE : SDL_FALSE);
	SDL_SetWindowGrab(Window, MouseIsGrabbed ? SDL_TRUE : SDL_FALSE);
#endif
}


/*-----------------------------------------------------------------------------
	Command line.
-----------------------------------------------------------------------------*/

static QSORT_RETURN CompareRes(const FPlane* A, const FPlane* B)
{
    return static_cast<QSORT_RETURN>((A->X - B->X) != 0.f ? (A->X - B->X) : (A->Y - B->Y));
}

//
// Command line.
//
UBOOL USDLViewport::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(USDLViewport::Exec);
    if(ParseCommand(&Cmd, TEXT("GETRES")))
    {
        if (RenDev && RenDev->Exec(Cmd, Ar))
            return 1;
        
        if (!Window)
            return 0;
        
        auto DisplayIndex = SDL_GetWindowDisplayIndex(Window);
        auto NumDisplayModes = SDL_GetNumDisplayModes(DisplayIndex);
        TArray<FPlane> ModeList;
        FString ModeListString;
        
        for (INT i = 0; i < NumDisplayModes; ++i)
        {
            SDL_DisplayMode Mode;
            if (SDL_GetDisplayMode(DisplayIndex, i, &Mode) == 0)
            {
                ModeList.AddUniqueItem(FPlane(Mode.w, Mode.h, 0, 0));
            }
        }
        
        if (ModeList.Num() > 0)
            appQsort(&ModeList(0), ModeList.Num(), sizeof(FPlane), reinterpret_cast<QSORT_COMPARE>(&CompareRes));
        for (INT i = 0; i < ModeList.Num(); ++i)
            ModeListString += FString::Printf(TEXT("%dx%d "), static_cast<INT>(ModeList(i).X), static_cast<INT>(ModeList(i).Y));
        Ar.Log(*ModeListString.LeftChop(1));
        return 1;
    }
	else if( UViewport::Exec( Cmd, Ar ) )
	{
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("ENDFULLSCREEN")) )
	{
		if( BlitFlags & BLIT_Fullscreen )
			EndFullscreen();
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("TOGGLEFULLSCREEN")) )
	{
		ToggleFullscreen();
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETCURRENTRES")) )
	{
		Ar.Logf( TEXT("%ix%i"), SizeX, SizeY ); // gam
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETCURRENTCOLORDEPTH")) )
	{
		Ar.Logf( TEXT("%i"), (ColorBytes?ColorBytes:2)*8 );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETCOLORDEPTHS")) )
	{
		Ar.Log( TEXT("32") ); //16
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETCURRENTRENDERDEVICE")) )
	{
		Ar.Log( RenDev->GetClass()->GetPathName() );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SUPPORTEDRESOLUTION")) )
	{
		INT		Width = 0,
				Height = 0,
				BitDepth = 0;
		UBOOL	Supported = 0;

		if(Parse(Cmd,TEXT("WIDTH="),Width) && Parse(Cmd,TEXT("HEIGHT="),Height) && Parse(Cmd,TEXT("BITDEPTH="),BitDepth))
		{
			Supported = 1;  // we scale to anything now.
		}

		Ar.Logf(TEXT("%u"),Supported);

		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GetScreenMode")) )
	{
		const Uint32 WindowFlags = SDL_GetWindowFlags(Window);
		if( (WindowFlags & SDL_WINDOW_FULLSCREEN_DESKTOP)==SDL_WINDOW_FULLSCREEN_DESKTOP )
			Ar.Log(TEXT("Borderless"));
		else if( IsFullscreen() )
			Ar.Log(TEXT("Fullscreen"));
		else
			Ar.Log(TEXT("Windowed"));
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SetScreenMode")) )
	{
		USDLClient* Client = GetOuterUSDLClient();
		const UBOOL WasFullscreenDesktop =
			(SDL_GetWindowFlags(Window) & SDL_WINDOW_FULLSCREEN_DESKTOP)==SDL_WINDOW_FULLSCREEN_DESKTOP;

		if( appStricmp(Cmd,TEXT("Fullscreen"))==0 )
		{
			Client->StartupFullscreen = 1;
			Client->BorderlessWindow = 0;
			Client->UseDesktopResolution = 0;
			if( !IsFullscreen() )
				ToggleFullscreen();
			else if( WasFullscreenDesktop && RenDev )
				RenDev->SetRes( INDEX_NONE, INDEX_NONE, ColorBytes, 1 );
			Client->SaveConfig();
		}
		else if( appStricmp(Cmd,TEXT("Windowed"))==0 )
		{
			Client->StartupFullscreen = 0;
			Client->BorderlessWindow = 0;
			Client->UseDesktopResolution = 0;
			if( IsFullscreen() )
				EndFullscreen();
			SDL_SetWindowBordered( Window, SDL_TRUE );
			Client->SaveConfig();
		}
		else if( appStricmp(Cmd,TEXT("Borderless"))==0 )
		{
			Client->StartupFullscreen = 1;
			Client->BorderlessWindow = 1;
			Client->UseDesktopResolution = 1;
			if( !WasFullscreenDesktop && RenDev )
				RenDev->SetRes( INDEX_NONE, INDEX_NONE, ColorBytes, 1 );
			Client->SaveConfig();
		}
		else
			return 0;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SETRES")) )
	{
		INT X=appAtoi(Cmd);
		const TCHAR* CmdTemp = appStrchr(Cmd,'x') ? appStrchr(Cmd,'x')+1 : appStrchr(Cmd,'X') ? appStrchr(Cmd,'X')+1 : TEXT("");
		INT Y=appAtoi(CmdTemp);
		Cmd = CmdTemp;
		CmdTemp = appStrchr(Cmd,'x') ? appStrchr(Cmd,'x')+1 : appStrchr(Cmd,'X') ? appStrchr(Cmd,'X')+1 : TEXT("");
		INT BPP=appAtoi(CmdTemp);
		Cmd = CmdTemp;
		UBOOL	Fullscreen = IsFullscreen() || FullscreenOnly;
		if(appStrchr(Cmd,'w') || appStrchr(Cmd,'W'))
			Fullscreen = 0;
		else if(appStrchr(Cmd,'f') || appStrchr(Cmd,'F'))
			Fullscreen = 1;
		if( X && Y )
		{
			INT ColorBytes = 0;
			switch( BPP )
			{
			case 16:
				ColorBytes = 2;
				break;
			case 24:
			case 32:
				ColorBytes = 4;
				break;
			}
            
            if (MouseScaleX > 1.f)
                X = appFloor(static_cast<FLOAT>(X) / MouseScaleX);
            if (MouseScaleY > 1.f)
                Y = appFloor(static_cast<FLOAT>(Y) / MouseScaleY);
			
			UBOOL Result = RenDev->SetRes( X, Y, ColorBytes, Fullscreen || FullscreenOnly);
			if( !Result )
				EndFullscreen();
			
			//if ( GUIController )
			//	GUIController->ResolutionChanged(X, Y);
		}
		return 1;
	}

	else if( ParseCommand(&Cmd,TEXT("TEMPSETRES")) )
	{
		INT X=appAtoi(Cmd);
		const TCHAR* CmdTemp = appStrchr(Cmd,'x') ? appStrchr(Cmd,'x')+1 : appStrchr(Cmd,'X') ? appStrchr(Cmd,'X')+1 : TEXT("");
		INT Y=appAtoi(CmdTemp);
		Cmd = CmdTemp;
		CmdTemp = appStrchr(Cmd,'x') ? appStrchr(Cmd,'x')+1 : appStrchr(Cmd,'X') ? appStrchr(Cmd,'X')+1 : TEXT("");
		INT BPP=appAtoi(CmdTemp);
		Cmd = CmdTemp;
		UBOOL	Fullscreen = IsFullscreen() || FullscreenOnly;
		if(appStrchr(Cmd,'w') || appStrchr(Cmd,'W'))
			Fullscreen = 0;
		else if(appStrchr(Cmd,'f') || appStrchr(Cmd,'F'))
			Fullscreen = 1;

		USDLClient*	Client = GetOuterUSDLClient();
		INT				SavedX,
						SavedY;
		if(Fullscreen)
		{
			SavedX = Client->FullscreenViewportX;
			SavedY = Client->FullscreenViewportY;
		}
		else
		{
			SavedX = Client->WindowedViewportX;
			SavedY = Client->WindowedViewportY;
		}

		if( X && Y )
		{
			INT ColorBytes = 0;
			switch( BPP )
			{
			case 16:
				ColorBytes = 2;
				break;
			case 24:
			case 32:
				ColorBytes = 4;
				break;
			}
			
			UBOOL Result = RenDev->SetRes( X, Y, ColorBytes, Fullscreen /*|| FullscreenOnly*/ );
			if( !Result )
				EndFullscreen();
			
//			if ( GUIController )
//				GUIController->ResolutionChanged(X, Y);
		}

		if(Fullscreen)
		{
			Client->FullscreenViewportX = SavedX;
			Client->FullscreenViewportY = SavedY;
		}
		else
		{
			Client->WindowedViewportX = SavedX;
			Client->WindowedViewportY = SavedY;
		}
		Client->SaveConfig();

		return 1;
	}

#if 0  // stijn: not implemented except on win32
	else if( ParseCommand(&Cmd,TEXT("PREFERENCES")) )
	{
		USDLClient* Client = GetOuterUSDLClient();
		Client->ConfigReturnFullscreen = 0;
		if( BlitFlags & BLIT_Fullscreen )
		{
			EndFullscreen();
			Client->ConfigReturnFullscreen = 1;
		}
		if( !Client->ConfigProperties )
		{
			Client->ConfigProperties = new WConfigProperties( TEXT("Preferences"), *LocalizeGeneralHelper("AdvancedOptionsTitle",TEXT("Window")) );
			Client->ConfigProperties->SetNotifyHook( Client );
			Client->ConfigProperties->OpenWindow( Window->hWnd );
			Client->ConfigProperties->ForceRefresh();
		}
		GetOuterUSDLClient()->ConfigProperties->Show(1);
		SetFocus( *GetOuterUSDLClient()->ConfigProperties );
		return 1;
	}
#else
	else if( ParseCommand(&Cmd,TEXT("PREFERENCES")) )
	{
		Ar.Log(TEXT("The preferences command is not available on your platform."));
		Ar.Log(TEXT("Please use the in-game menu or edit the following files to modify your game preferences:"));
		Ar.Logf(TEXT("    > %s"), dynamic_cast<FConfigCacheIni*>(GConfig) ? *dynamic_cast<FConfigCacheIni*>(GConfig)->SystemIni : TEXT("UnrealTournament.ini"));
		Ar.Logf(TEXT("    > %s"), dynamic_cast<FConfigCacheIni*>(GConfig) ? *dynamic_cast<FConfigCacheIni*>(GConfig)->UserIni : TEXT("User.ini"));
		return 1;
	}
#endif
	else if ( ParseCommand(&Cmd,TEXT("GETSYSTEMINI")) )
	{
		Ar.Logf(TEXT("%ls"), dynamic_cast<FConfigCacheIni*>(GConfig) ? *dynamic_cast<FConfigCacheIni*>(GConfig)->SystemIni : TEXT("UnrealTournament.ini"));
		return 1;
	}
	else if ( ParseCommand(&Cmd,TEXT("GETUSERINI")) )
	{
		Ar.Logf(TEXT("%ls"), dynamic_cast<FConfigCacheIni*>(GConfig) ? *dynamic_cast<FConfigCacheIni*>(GConfig)->UserIni : TEXT("User.ini"));
		return 1;
	}
	else if ( ParseCommand(&Cmd,TEXT("RELAUNCHSUPPORT")) )
	{
		Ar.Logf(TEXT("DISABLED"));
		return 1;
	}
	else if ( ParseCommand(&Cmd,TEXT("LOGWINDOWSUPPORT")) )
	{
		Ar.Logf(TEXT("DISABLED"));
		return 1;
	}

	// gam ---
	else if( ParseCommand(&Cmd,TEXT("SETMOUSE")) )
	{
		const TCHAR* CmdTemp = Cmd;
		INT X, Y;
		
		X = appAtoi(CmdTemp);
		
		while( appIsDigit( *CmdTemp) )
		    CmdTemp++;

		while( isspace( *CmdTemp) )
		    CmdTemp++;
		    
		Y = appAtoi(CmdTemp);
		
		SDL_WarpMouseInWindow(Window, X, Y);
		return 1;
	}

	else if( ParseCommand(&Cmd,TEXT("TTS")) )
	{
		if( appStrcmp(Cmd,TEXT("")) != 0 )
			TextToSpeech( FString(Cmd), 1.f );
		return 1;
	}

	// vi represent!  --ryan.
	else if ((ParseCommand(&Cmd,TEXT(":wq"))) || (ParseCommand(&Cmd,TEXT(":q!"))))
	{
		GIsRequestingExit = 1;
		return 1;
	}

	// --- gam

	return 0;
	unguard;
}

/*-----------------------------------------------------------------------------
	Window openining and closing.
-----------------------------------------------------------------------------*/

//
// Open this viewport's window.
//
void USDLViewport::OpenWindow( DWORD ParentWindow, UBOOL IsTemporary, INT NewX, INT NewY, INT OpenX, INT OpenY )
{
	guard(USDLViewport::OpenWindow);
	void* InParentWindow = HostPointerFromLegacyHandle(ParentWindow);
	(void)InParentWindow;

    // Can't do multiple windows with SDL 1.2.
    // Eh...Splash screen. !!! FIXME.
    //check(SDL_GetVideoSurface() == NULL);

	// Check resolution.
	if (GIsEditor)
	{
		SDL_DisplayMode mode;
		SDL_GetDesktopDisplayMode(0, &mode);
		if (SDL_BITSPERPIXEL(mode.format) < 24)
			appErrorf(TEXT("Editor requires desktop set to 32 bit resolution"));
	}

	check(Actor);
	UBOOL DoRepaint=0, DoSetActive=0;
	USDLClient* C = GetOuterUSDLClient();

    if (TextToSpeechObject == -1)
    {
        if (C->TextToSpeechFile.Len() == 0)
            debugf(TEXT("TTS: No output filename specified."));
        else
        {
            // Please note we don't create this file...
            TextToSpeechObject = open(appToAnsi(*(C->TextToSpeechFile)), O_WRONLY);
            if (TextToSpeechObject != -1)
                debugf(TEXT("TTS: Opened file \"%s\" for text-to-speech output."), *(C->TextToSpeechFile));
            else
            {
                int err = errno;
                debugf(TEXT("TTS: Couldn't open TTS file \"%s\""), *(C->TextToSpeechFile));
                debugf(TEXT("TTS: System error is \"%s\" (%d)."), appFromAnsi(strerror(err)), err);
                if (err == ENOENT)
                {
                    debugf(TEXT("TTS: (We intentionally don't create this file if it doesn't exist!)"));
                    debugf(TEXT("TTS: Disabling Text-to-speech support..."));
                }
            }
        }
    }

    #if MACOSX
    if (!MacTextToSpeechEnabled)
    {
        if (C->MacNativeTextToSpeech)
        {
            int rc = (int) NewSpeechChannel(NULL, &MacTextToSpeechChannel);
            if (rc != 0)
                debugf(TEXT("TTS: NewSpeechChannel() failed! rc==%d"), rc);
            else
            {
                debugf(TEXT("TTS: Native MacOS X text-to-speech enabled."));
                MacTextToSpeechEnabled = 1;
            }
        }

        if (!MacTextToSpeechEnabled)
            debugf(TEXT("TTS: Native MacOS X text-to-speech is NOT enabled."));
    }
    #endif

	debugf( TEXT("Opening SDL viewport.") );

	// Create or update the window.
	SizeX = C->FullscreenViewportX;
	SizeY = C->FullscreenViewportY;

	const UBOOL StartupFullscreen = C->StartupFullscreen && !ParseParam(appCmdLine(),TEXT("window"));
	// Create rendering device.
	if( !RenDev && !GIsEditor && !ParseParam(appCmdLine(),TEXT("nohard")) )
		TryRenderDevice( TEXT("ini:Engine.Engine.GameRenderDevice"), NewX, NewY, ColorBytes, StartupFullscreen );
	check(RenDev);
	UpdateWindowFrame();
	Repaint( 1 );

	unguard;
}

//
// Close a viewport window.  Assumes that the viewport has been opened with
// OpenViewportWindow.  Does not affect the viewport's object, only the
// platform-specific information associated with it.
//
void USDLViewport::CloseWindow()
{
	guard(USDLViewport::CloseWindow);
    /* no-op. */
	unguard;
}

/*-----------------------------------------------------------------------------
	USDLViewport operations.
-----------------------------------------------------------------------------*/

//
// Set window position according to menu's on-top setting:
//
void USDLViewport::SetTopness()
{
	guard(USDLViewport::SetTopness);
    /* no-op. */
	unguard;
}

//
// Repaint the viewport.
//
void USDLViewport::Repaint( UBOOL Blit )
{
	guard(USDLViewport::Repaint);
	GetOuterUSDLClient()->Engine->Draw( this, Blit );
	unguard;
}

//
// Return whether fullscreen.
//
UBOOL USDLViewport::IsFullscreen()
{
	guard(USDLViewport::IsFullscreen);
	return (BlitFlags & BLIT_Fullscreen)!=0;
	unguard;
}

//
// Set the mouse cursor according to Unreal or UnrealEd's mode, or to
// an hourglass if a slow task is active.
//
void USDLViewport::SetModeCursor()
{
	guard(USDLViewport::SetModeCursor);

// !!! FIXME: What to do with this?
#if 0 // original WinDrv code follows:
	if( GIsSlowTask )
	{
		SetCursor(LoadCursorIdX(NULL,IDC_WAIT));
		return;
	}
	HCURSOR hCursor = NULL;
	switch( GetOuterUSDLClient()->Engine->edcamMode(this) )
	{
		case EM_ViewportZoom:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_CameraZoom); break;
		case EM_ActorRotate:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_BrushRot); break;
		case EM_ActorScale:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_BrushScale); break;
		case EM_ActorSnapScale:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_BrushSnap); break;
		case EM_TexturePan:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_TexPan); break;
		case EM_TextureRotate:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TexRot); break;
		case EM_TextureScale:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TexScale); break;
		case EM_None: 				hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		case EM_ViewportMove:		hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		case EM_TexView:			hCursor = LoadCursorIdX(NULL,IDC_ARROW); break;
		case EM_TexBrowser:			hCursor = LoadCursorIdX(NULL,IDC_ARROW); break;
		case EM_StaticMeshBrowser:	hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		case EM_MeshView:			hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		case EM_Animation:			hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		case EM_VertexEdit:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_VertexEdit); break;
		case EM_BrushClip:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_BrushClip); break;
		case EM_FaceDrag:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_FaceDrag); break;
		case EM_Polygon:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_BrushWarp); break;
		case EM_TerrainEdit:
		{
			switch( GetOuterUSDLClient()->Engine->edcamTerrainBrush() )
			{
				case TB_VertexEdit:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_VertexEdit); break;
				case TB_Paint:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Paint); break;
				case TB_Smooth:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Smooth); break;
				case TB_Noise:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Noise); break;
				case TB_Flatten:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Flatten); break;
				case TB_TexturePan:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TexPan); break;
				case TB_TextureRotate:	hCursor = LoadCursorIdX(hInstance,IDCURSOR_TexRot); break;
				case TB_TextureScale:	hCursor = LoadCursorIdX(hInstance,IDCURSOR_TexScale); break;
				case TB_Select:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Selection); break;
				case TB_Visibility:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Visibility); break;
				case TB_Color:			hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_Color); break;
				case TB_EdgeTurn:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_TB_EdgeTurn); break;
				default:				hCursor = LoadCursorIdX(hInstance,IDCURSOR_TerrainEdit); break;
			}
		}
		break;
		case EM_PrefabBrowser:	hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		case EM_Matinee:	 	hCursor = LoadCursorIdX(hInstance,IDCURSOR_Matinee); break;
		case EM_EyeDropper:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_EyeDropper); break;
		case EM_FindActor:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_FindActor); break;
		case EM_Geometry:		hCursor = LoadCursorIdX(hInstance,IDCURSOR_Geometry); break;
		
		case EM_NewCameraMove:	hCursor = LoadCursorIdX(NULL,IDC_CROSS); break;
		default: 				hCursor = LoadCursorIdX(NULL,IDC_ARROW); break;
	}
	check(hCursor);
	SetCursor( hCursor );
#endif

	unguard;
}


void USDLViewport::SetTitleBar()
{
	FString WindowName;
	
	// Set viewport window's name to show resolution.
	if( !GIsEditor || (Actor->ShowFlags&SHOW_PlayerCtrl) )
	{
		WindowName = TEXT("Harry Potter and the Chamber of Secrets");
	}
	else switch( Actor->RendMap )
	{
		case REN_Wire:		WindowName = *LocalizeGeneralHelper(TEXT("ViewPersp"),TEXT("WinDrv")); break;
		case REN_OrthXY:	WindowName = *LocalizeGeneralHelper(TEXT("ViewXY")   ,TEXT("WinDrv")); break;
		case REN_OrthXZ:	WindowName = *LocalizeGeneralHelper(TEXT("ViewXZ")   ,TEXT("WinDrv")); break;
		case REN_OrthYZ:	WindowName = *LocalizeGeneralHelper(TEXT("ViewYZ")   ,TEXT("WinDrv")); break;
		default:	   WindowName = *LocalizeGeneralHelper(TEXT("ViewOther"),TEXT("WinDrv")); break;
	}

	SDL_SetWindowTitle(Window, appToAnsi(*WindowName));

#if !MACOSX
    static bool loaded_icon = false;
    if (!loaded_icon)
    {
        loaded_icon = true;
        SDL_Surface *icon = SDL_LoadBMP("../Help/Unreal.bmp");
        if (icon != NULL)
            SDL_SetWindowIcon(Window, icon);
    }
#endif
}


//
// Update user viewport interface.
//
void USDLViewport::UpdateWindowFrame()
{
	guard(USDLViewport::UpdateWindowFrame);

	// If not a window, exit.
	if( !Window || (BlitFlags&BLIT_Fullscreen) || (BlitFlags&BLIT_Temporary) || !Actor)
		return;

    SetTitleBar();

	unguard;
}

//
// Return the viewport's window.
//
void* USDLViewport::GetWindow()
{
	guard(USDLViewport::GetWindow);
	return Window;
	unguard;
}

/*-----------------------------------------------------------------------------
	Input.
-----------------------------------------------------------------------------*/

static const EInputKey JoystickAxisKeys[USDLClient::MaxJoystickAxes] =
{
	IK_JoyX,
	IK_JoyY,
	IK_JoyZ,
	IK_JoyR,
	IK_JoyU,
	IK_JoyV,
	IK_UnknownEA,
	IK_UnknownEB
};

static FLOAT NormalizeJoystickAxis(Sint16 Value, FLOAT DeadZone)
{
	const FLOAT Normalized = Value < 0
		? (FLOAT)Value / 32768.0f
		: (FLOAT)Value / 32767.0f;
	const FLOAT ClampedDeadZone = Clamp(DeadZone, 0.0f, 0.99f);
	const FLOAT Magnitude = Normalized < 0.0f ? -Normalized : Normalized;
	if (Magnitude <= ClampedDeadZone)
		return 0.0f;

	const FLOAT RemappedMagnitude = (Magnitude - ClampedDeadZone) / (1.0f - ClampedDeadZone);
	return Normalized < 0.0f ? -RemappedMagnitude : RemappedMagnitude;
}

//
// Input event router.
//
UBOOL USDLViewport::CauseInputEvent( INT iKey, EInputAction Action, FLOAT Delta )
{
	guard(USDLViewport::CauseInputEvent);
	return GetOuterUSDLClient()->Engine->InputEvent( this, (EInputKey)iKey, Action, Delta );
	unguard;
}
void USDLViewport::ReleaseJoystickInput()
{
	if (Input != NULL)
	{
		for (INT ButtonIndex = 0; ButtonIndex < USDLClient::JoystickButtons; ++ButtonIndex)
		{
			const INT Button = IK_Joy1 + ButtonIndex;
			if (Input->KeyDown(Button))
				CauseInputEvent(Button, IST_Release);
		}

		for (INT AxisIndex = 0; AxisIndex < USDLClient::JoystickAxes; ++AxisIndex)
			CauseInputEvent(JoystickAxisKeys[AxisIndex], IST_Axis, 0.0f);

		if (LastJoyHat != IK_None && Input->KeyDown(LastJoyHat))
			CauseInputEvent(LastJoyHat, IST_Release);
	}

	LastJoyHat = IK_None;
}

void USDLViewport::ReleaseAllInput()
{
	if (Input != NULL)
	{
		for (INT Key = 0; Key < IK_MAX; ++Key)
			if (Input->KeyDown(Key))
				CauseInputEvent(Key, IST_Release);

		for (INT AxisIndex = 0; AxisIndex < USDLClient::MaxJoystickAxes; ++AxisIndex)
			CauseInputEvent(JoystickAxisKeys[AxisIndex], IST_Axis, 0.0f);

		CauseInputEvent(IK_MouseX, IST_Axis, 0.0f);
		CauseInputEvent(IK_MouseY, IST_Axis, 0.0f);
		CauseInputEvent(IK_MouseW, IST_Axis, 0.0f);

		if (Actor != NULL)
		{
			for (TFieldIterator<UByteProperty> ItB(Actor->GetClass()); ItB; ++ItB)
				if (ItB->PropertyFlags & CPF_Input)
					*(BYTE*)((BYTE*)Actor + ItB->Offset) = 0;

			for (TFieldIterator<UFloatProperty> ItF(Actor->GetClass()); ItF; ++ItF)
				if (ItF->PropertyFlags & CPF_Input)
					*(FLOAT*)((BYTE*)Actor + ItF->Offset) = 0.0f;
		}
	}

	LastJoyHat = IK_None;
}


//
// If the cursor is currently being captured, stop capturing, clipping, and 
// hiding it, and move its position back to where it was when it was initially
// captured.
//
void USDLViewport::SetMouseCapture( UBOOL Capture, UBOOL Clip, UBOOL OnlyFocus )
{
	guard(USDLViewport::SetMouseCapture);

	bWindowsMouseAvailable = !Capture;
	UpdateMouseGrabState(Capture);

	unguard;
}

int ucs2_char_code_from_utf8(char const *utf8_char_seq)
{
    unsigned char c0, c1, c2;
    c0 = *utf8_char_seq;

    if (c0 < 128)
        return c0;
    if (c0 < 224)
    {
        c1 = utf8_char_seq[1];
        if (128 <= c1 && c1 < 192)
            return (c0 & 31) << 6 | (c1 & 63);
        return -1;
    }
    if (c0 < 240)
    {
        c1 = utf8_char_seq[1];
        if (128 <= c1 && c1 < 192)
        {
            c2 = utf8_char_seq[2];
            if (128 <= c2 && c2 < 192)
                return (c0 & 15) << 12 | (c1 & 63) << 6 | (c2 & 63);
        }
        return -1;
    }
    return -1;
}


//
// Update input for this viewport.
//
void USDLViewport::UpdateInput( UBOOL Reset )
{
	guard(USDLViewport::UpdateInput);

	UpdateSpeech();

	FTime UpdateTime = appSeconds();
	FLOAT DeltaSeconds = UpdateTime - LastUpdateTime;
	LastUpdateTime = UpdateTime;
  
	USDLClient* Client = GetOuterUSDLClient();

	if (USDLClient::Joystick != NULL && !SDL_JoystickGetAttached(USDLClient::Joystick))
	{
		ReleaseJoystickInput();
		Client->CloseJoystick();
		if (Client->UseJoystick)
			Client->OpenJoystick();
	}

	const UBOOL HasInputFocus = Window != NULL
		&& (SDL_GetWindowFlags(Window) & SDL_WINDOW_INPUT_FOCUS) != 0;
	if (Client->UseJoystick && USDLClient::Joystick != NULL && HasInputFocus)
	{
		SDL_JoystickUpdate();

		for (INT ButtonIndex = 0; ButtonIndex < USDLClient::JoystickButtons; ++ButtonIndex)
		{
			const Uint8 State = SDL_JoystickGetButton(USDLClient::Joystick, ButtonIndex);
			const INT Button = IK_Joy1 + ButtonIndex;
			if (!Input->KeyDown(Button) && State == SDL_PRESSED)
				CauseInputEvent(Button, IST_Press);
			else if (Input->KeyDown(Button) && State != SDL_PRESSED)
				CauseInputEvent(Button, IST_Release);
		}

		check(USDLClient::JoystickAxes <= USDLClient::MaxJoystickAxes);
		for (INT AxisIndex = 0; AxisIndex < USDLClient::JoystickAxes; ++AxisIndex)
		{
			const Sint16 RawPosition = SDL_JoystickGetAxis(USDLClient::Joystick, AxisIndex);
			const FLOAT Position = NormalizeJoystickAxis(RawPosition, Client->JoystickDeadZone);
			CauseInputEvent(JoystickAxisKeys[AxisIndex], IST_Axis, Position);
		}
	}

	// Keyboard.
	//EInputKey Key 	    = IK_None;
	//EInputKey UpperCase = IK_None;
	//EInputKey LowerCase = IK_None;

	// Mouse movement management.
	UBOOL MouseMoved = false;
	UBOOL JoyHatMoved = false;
	INT DX = 0, DY = 0; // unscaled coords
	INT AbsX = 0, AbsY = 0; // unscaled coords
	EInputKey ThisJoyHat = IK_None;

	
	SDL_Event Event;
	while( SDL_PollEvent( &Event ) )
	{
		switch( Event.type )
		{
			case SDL_QUIT:
				GIsRequestingExit = 1;
				break;

			case SDL_KEYDOWN:
				if ( 0 && (Event.key.keysym.sym == SDLK_g) && (Event.key.keysym.mod & KMOD_CTRL) )
				{
					UpdateMouseGrabState(!MouseIsGrabbed);
					break;  // Don't pass event to engine.
				}

				else if ( (Event.key.keysym.sym == SDLK_RETURN) && (Event.key.keysym.mod & KMOD_ALT) )
				{
					ToggleFullscreen();
					break;  // Don't pass event to engine.
				}

				else
				{
					// Get key code.
					const BYTE *pInputKey = KeysymMap.Find(Event.key.keysym.sym);
					const EInputKey Key = pInputKey ? (EInputKey) *pInputKey : IK_None;

					// Send key to input system.
					if (Key != IK_None)
						CauseInputEvent( Key, IST_Press );
				}
				break;

			case SDL_KEYUP: {
				// Get key code.
				const BYTE *pInputKey = KeysymMap.Find(Event.key.keysym.sym);
				const EInputKey Key = pInputKey ? (EInputKey) *pInputKey : IK_None;

				// Send key to input system.
				if (Key != IK_None)
					CauseInputEvent( Key, IST_Release );
				break;
            }

			case SDL_TEXTINPUT:
			{
				// here we go, unicode textinput.
                auto KeyValue = ucs2_char_code_from_utf8(Event.text.text);
				
				if (KeyValue != IK_None)
                    Client->Engine->Key(this, (EInputKey)KeyValue
#if ENGINE_VERSION==227
										, KeyValue
#endif
						);
			}
			break;

			case SDL_MOUSEBUTTONDOWN:
				switch (Event.button.button)
				{
					case 1: CauseInputEvent( IK_LeftMouse, IST_Press ); break;
					case 2: CauseInputEvent( IK_MiddleMouse, IST_Press ); break;
					case 3: CauseInputEvent( IK_RightMouse, IST_Press ); break;
				}
				break;

			case SDL_MOUSEBUTTONUP:
				switch (Event.button.button)
				{
					case 1: CauseInputEvent( IK_LeftMouse, IST_Release ); break;
					case 2: CauseInputEvent( IK_MiddleMouse, IST_Release ); break;
					case 3: CauseInputEvent( IK_RightMouse, IST_Release ); break;
				}
				break;

			case SDL_MOUSEWHEEL:
				if (Event.wheel.y)
				{
					CauseInputEvent( IK_MouseW, IST_Axis, Event.wheel.y );
					if( Event.wheel.y < 0 )
					{
						CauseInputEvent( IK_MouseWheelDown, IST_Press );
						CauseInputEvent( IK_MouseWheelDown, IST_Release );
					}
					else
					{
						CauseInputEvent( IK_MouseWheelUp, IST_Press );
						CauseInputEvent( IK_MouseWheelUp, IST_Release );
					}
				}
				break;

			case SDL_MOUSEMOTION:
				MouseMoved = true;
				AbsX = Event.motion.x;
				AbsY = Event.motion.y;
				DX += Event.motion.xrel;
				DY += Event.motion.yrel;

//				debugf(TEXT("MOUSEMOTION %d,%d"), Event.motion.xrel, Event.motion.yrel);
				if (!MouseIsGrabbed && 
					SelectedCursor >= 0 && 
					SelectedCursor <= ARRAY_COUNT(StandardCursors) &&
					StandardCursors[SelectedCursor])
				{
					SDL_SetCursor(StandardCursors[SelectedCursor]);
					ActiveCursor = StandardCursors[SelectedCursor];
				}
				break;
	
			case SDL_JOYDEVICEADDED:
				if (Client->UseJoystick && USDLClient::Joystick == NULL)
					Client->OpenJoystick();
				break;

			case SDL_JOYDEVICEREMOVED:
				if (USDLClient::Joystick != NULL
					&& Event.jdevice.which == USDLClient::JoystickInstanceId)
				{
					ReleaseJoystickInput();
					Client->CloseJoystick();
					if (Client->UseJoystick)
						Client->OpenJoystick();
				}
				break;

			case SDL_JOYBALLMOTION:
				if (USDLClient::Joystick == NULL
					|| Event.jball.which != USDLClient::JoystickInstanceId
					|| !Client->UseJoystick)
					break;
				MouseMoved = true;
				DX += Event.jball.xrel * Client->ScaleJBX;
				DY += Event.jball.yrel * Client->ScaleJBY;
				break;
	
			case SDL_JOYHATMOTION:
				if ( (USDLClient::Joystick == NULL) ||
				     (Event.jhat.which != USDLClient::JoystickInstanceId) ||
				     (Event.jhat.hat != Client->JoystickHatNumber) ||
				     (!Client->UseJoystick) ||
				     (Client->IgnoreHat) )
				{
					break;
				}

				JoyHatMoved = true;

				switch ( Event.jhat.value )
				{
					case SDL_HAT_UP :
						ThisJoyHat = IK_Joy13;
						break;
					case SDL_HAT_DOWN :
						ThisJoyHat = IK_Joy14;
						break;
					case SDL_HAT_LEFT :
						ThisJoyHat = IK_Joy15;
						break;
					case SDL_HAT_RIGHT :
						ThisJoyHat = IK_Joy16;
						break;
					default :
						ThisJoyHat = (EInputKey) 0;
						break;
				}				
				break;

	        case SDL_WINDOWEVENT:
				if (Event.window.event == SDL_WINDOWEVENT_ENTER)
				{
					// resynchronize mouse position
					GetMouseState(&MouseEnterX, &MouseEnterY);
					MouseMoved = true;
					AbsX = MouseEnterX;
					AbsY = MouseEnterY;
				}					
				else if (Event.window.event == SDL_WINDOWEVENT_LEAVE)
				{
					GetMouseState(&MouseLeaveX, &MouseLeaveY);
					MouseMoved = true;
					AbsX = MouseLeaveX;
					AbsY = MouseLeaveY;
				}
				else if (Event.window.event == SDL_WINDOWEVENT_RESIZED)
				{
					InResize = 1;
					INT NewX = (IsFullscreen() ? 1 : MouseScaleX) * Event.window.data1;
					INT NewY = (IsFullscreen() ? 1 : MouseScaleY) * Event.window.data2;

					// stijn: macOS Retina support
					if (RenDev && (BlitFlags & (BLIT_OpenGL|BLIT_Vulkan)))
					{
						INT WindowPointSizeX, WindowPointSizeY, WindowPixelSizeX, WindowPixelSizeY;
						SDL_GetWindowSize(Window, &WindowPointSizeX, &WindowPointSizeY);
                        if (BlitFlags & (BLIT_OpenGL|BLIT_Vulkan))
                            SDL_GL_GetDrawableSize(Window, &WindowPixelSizeX, &WindowPixelSizeY);
                        else
                            SDL_Metal_GetDrawableSize(Window, &WindowPixelSizeX, &WindowPixelSizeY);

						debugf(TEXT("SDL_WINDOWEVENT_RESIZED: %dx%d - Effective Point Size: %dx%d - GLPixel Size %dx%d"),
							   NewX, NewY, WindowPointSizeX, WindowPointSizeY, WindowPixelSizeX, WindowPixelSizeY);

						if (WindowPixelSizeX > NewX || WindowPixelSizeY > NewY)
						{
							NewX = WindowPixelSizeX;
							NewY = WindowPixelSizeY;
						}
					}

					ResizeViewport(BlitFlags|BLIT_NoWindowChange, NewX, NewY, ColorBytes);
					
					if (GIsEditor)
						Repaint(0);

					InResize = 0;
				}
				else if ((Event.window.event == SDL_WINDOWEVENT_FOCUS_LOST
					|| Event.window.event == SDL_WINDOWEVENT_MINIMIZED
					|| Event.window.event == SDL_WINDOWEVENT_HIDDEN)
					&& Window != NULL
					&& Event.window.windowID == SDL_GetWindowID(Window))
				{
					if (!LostGrab && MouseIsGrabbed)
					{
						LostGrab = 1;
						UpdateMouseGrabState(0);
					}
					ReleaseAllInput();
					INT IgnoredRelativeX, IgnoredRelativeY;
					SDL_GetRelativeMouseState(&IgnoredRelativeX, &IgnoredRelativeY);
					MouseMoved = false;
					JoyHatMoved = false;
					DX = DY = 0;
					ThisJoyHat = IK_None;
				}
				else if ((Event.window.event == SDL_WINDOWEVENT_FOCUS_GAINED
					|| Event.window.event == SDL_WINDOWEVENT_RESTORED
					|| Event.window.event == SDL_WINDOWEVENT_SHOWN)
					&& Window != NULL
					&& Event.window.windowID == SDL_GetWindowID(Window))
				{
					const Uint32 WindowFlags = SDL_GetWindowFlags(Window);
					if (LostGrab
						&& (WindowFlags & SDL_WINDOW_INPUT_FOCUS) != 0
						&& (WindowFlags & (SDL_WINDOW_MINIMIZED | SDL_WINDOW_HIDDEN)) == 0)
					{
						UpdateMouseGrabState(GetOuterUSDLClient()->CaptureMouse);
						LostGrab = 0;
					}
				}
				break;

			default:;
		}
	}

	// Deliver mouse behavior to the engine.
	if ( MouseMoved )
	{
		if (!MouseIsGrabbed)
		{
			DX = AbsX * MouseScaleX - WindowsMouseX;
			DY = AbsY * MouseScaleY - WindowsMouseY;
		}
		
		// Send to input subsystem.
		if( DX )
			CauseInputEvent( IK_MouseX, IST_Axis, +DX );
		
		if( DY )
			CauseInputEvent( IK_MouseY, IST_Axis, -DY );
		
		if (!Client->InMenuLoop)
			Client->Engine->MouseDelta(this, 0, DX, DY);
		
		if (!IsRealtime())
		{
			if( Input->KeyDown(IK_Space) )
			{
				for( INT i=0; i<Client->Viewports.Num(); i++ )
					Client->Viewports(i)->Repaint( 1 );
			}
			else
			{
				Repaint( 1 );
			}
		}
		
		if (!MouseIsGrabbed)
		{
			WindowsMouseX = Clamp(AbsX * MouseScaleX, 0.0, (DOUBLE)SizeX);
			WindowsMouseY = Clamp(AbsY * MouseScaleY, 0.0, (DOUBLE)SizeY);
		}
	}

	if ( (LastJoyHat != ThisJoyHat) && JoyHatMoved )
	{
		if (LastJoyHat)
			CauseInputEvent( LastJoyHat, IST_Release );
		if (ThisJoyHat)	
			CauseInputEvent( ThisJoyHat, IST_Press );		
		LastJoyHat = ThisJoyHat;
	} 

	unguard;
}



/*-----------------------------------------------------------------------------
	Lock and Unlock.
-----------------------------------------------------------------------------*/

//
// Lock the viewport window and set the approprite Screen and RealScreen fields
// of Viewport.  Returns 1 if locked successfully, 0 if failed.  Note that a
// lock failing is not a critical error; it's a sign that a DirectDraw mode
// has ended or the user has closed a viewport window.
//
UBOOL USDLViewport::Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )
{
	guard(USDLViewport::LockWindow);
	USDLClient* Client = GetOuterUSDLClient();
	clock(Client->DrawCycles);

	// Success here, so pass to superclass.
	unclock(Client->DrawCycles);
	return UViewport::Lock(FlashScale,FlashFog,ScreenClear,RenderLockFlags,HitData,HitSize);

	unguard;
}

//
// Unlock the viewport window.  If Blit=1, blits the viewport's frame buffer.
//
void USDLViewport::Unlock( UBOOL Blit )
{
	guard(USDLViewport::Unlock);

	UViewport::Unlock( Blit );

	unguard;
}

/*-----------------------------------------------------------------------------
	Viewport modes.
-----------------------------------------------------------------------------*/

//
// Try switching to a new rendering device.
//
void USDLViewport::TryRenderDevice( const TCHAR* ClassName, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )
{
	guard(USDLViewport::TryRenderDevice);

	// Shut down current rendering device.
	if( RenDev )
	{
		RenDev->Exit();
		delete RenDev;
		RenDev = NULL;
	}

	// Use appropriate defaults.
	USDLClient* C = GetOuterUSDLClient();
	if( NewX==INDEX_NONE )
		NewX = Fullscreen ? C->FullscreenViewportX : C->WindowedViewportX;
	if( NewY==INDEX_NONE )
		NewY = Fullscreen ? C->FullscreenViewportY : C->WindowedViewportY;

	// Find device driver.
#if UNREAL_TOURNAMENT_OLDUNREAL
	UClass* RenderClass = UObject::StaticLoadClass( URenderDeviceOldUnreal469::StaticClass(), NULL, ClassName, NULL, 0, NULL );
#else
	UClass* RenderClass = UObject::StaticLoadClass( URenderDevice::StaticClass(), NULL, ClassName, NULL, 0, NULL );
#endif
	if( RenderClass )
	{
		debugf( TEXT("Loaded render device class.") );
		//HoldCount++;
#if UNREAL_TOURNAMENT_OLDUNREAL
		RenDev = ConstructObject<URenderDeviceOldUnreal469>( RenderClass, this );
#else
		RenDev = ConstructObject<URenderDevice>( RenderClass, this );
#endif
		if( RenDev->Init( this, NewX, NewY, NewColorBytes, Fullscreen ) )
		{
			if( GIsRunning )
				Actor->GetLevel()->DetailChange( RenDev->HighDetailActors );
		}
		else
		{
#if ENGINE_VERSION==227
			debugf( NAME_Log, *LocalizeError("Failed3D") );
#else
			debugf( NAME_Log, LocalizeError("Failed3D") );
#endif
			delete RenDev;
			RenDev = NULL;
		}
		//HoldCount--;
	}

	GRenderDevice = RenDev;
	unguard;
}


//
// If in fullscreen mode, end it and return to Windows.
//
void USDLViewport::EndFullscreen()
{
	guard(USDLViewport::EndFullscreen);
	//USDLClient* Client = GetOuterUSDLClient();
	debugf(NAME_Log, TEXT("Ending fullscreen mode by request."));
	if( RenDev && RenDev->FullscreenOnly )
	{
		// This device doesn't support fullscreen, so use a window-capable rendering device.
		TryRenderDevice( TEXT("ini:Engine.Engine.WindowedRenderDevice"), INDEX_NONE, INDEX_NONE, ColorBytes, 0 );
		check(RenDev);
	}
	else if( RenDev && (BlitFlags & (BLIT_OpenGL|BLIT_Vulkan)) )
	{
		RenDev->SetRes( INDEX_NONE, INDEX_NONE, ColorBytes, 0 );	
	}
	else
	{
		ResizeViewport( BLIT_DibSection );
	}
	UpdateWindowFrame();

	if ( Input )
		Input->ResetInput();

	unguard;
}

//
// Toggle fullscreen.
//
void USDLViewport::ToggleFullscreen()
{
	guard(USDLViewport::ToggleFullscreen);
	guard(USDLViewport::ToggleFullscreen);
	if( BlitFlags & BLIT_Fullscreen )
	{
		EndFullscreen();
	}
	else if( !(Actor->ShowFlags & SHOW_ChildWindow) )
	{
		debugf(TEXT("AttemptFullscreen"));
		TryRenderDevice( TEXT("ini:Engine.Engine.GameRenderDevice"), INDEX_NONE, INDEX_NONE, ColorBytes, 1 );
		if( !RenDev )
			TryRenderDevice( TEXT("ini:Engine.Engine.WindowedRenderDevice"), INDEX_NONE, INDEX_NONE, ColorBytes, 1 );
		if( !RenDev )
			TryRenderDevice( TEXT("ini:Engine.Engine.WindowedRenderDevice"), INDEX_NONE, INDEX_NONE, ColorBytes, 0 );
	}
	unguard;
	unguard;
}

static Uint32 GetFullscreenMode( UBOOL UseDesktopResolution )
{
	return UseDesktopResolution ? SDL_WINDOW_FULLSCREEN_DESKTOP : SDL_WINDOW_FULLSCREEN;
}

//
// Resize the viewport.
//
UBOOL USDLViewport::ResizeViewport( DWORD NewBlitFlags, INT InNewX, INT InNewY, UBOOL bSaveSize )
{
	guard(USDLViewport::ResizeViewport);
	USDLClient* Client = GetOuterUSDLClient();

	// Andrew says: I made this keep the BLIT_Temporary flag so the temporary screen buffer doesn't get leaked
	// during light rebuilds.
	
	// Remember viewport.
	UViewport* SavedViewport = NULL;
	if( Client->Engine->Audio && !GIsEditor && !(GetFlags() & RF_Destroyed) )
		SavedViewport = Client->Engine->Audio->GetViewport();

	UBOOL RequestFullscreen = (NewBlitFlags & BLIT_Fullscreen);
	
	// Default resolution handling.
	INT NewX = InNewX != INDEX_NONE ? InNewX : RequestFullscreen ? Client->FullscreenViewportX : Client->WindowedViewportX;
	INT NewY = InNewY != INDEX_NONE ? InNewY : RequestFullscreen ? Client->FullscreenViewportY : Client->WindowedViewportY;
	// Borderless desktop is an explicit client mode, not exclusive fullscreen
	// at the last selected resolution. Match the active display before choosing
	// SDL_WINDOW_FULLSCREEN_DESKTOP so the window cannot change display mode.
	if( RequestFullscreen && Client->UseDesktopResolution )
	{
		SDL_DisplayMode DesktopMode{};
		const int DisplayIndex = Window ? SDL_GetWindowDisplayIndex(Window) : 0;
		if( SDL_GetDesktopDisplayMode(DisplayIndex, &DesktopMode) == 0 )
		{
			NewX = DesktopMode.w;
			NewY = DesktopMode.h;
		}
	}

	UBOOL IsFullscreen = (Window && (SDL_GetWindowFlags(Window) & (SDL_WINDOW_FULLSCREEN | SDL_WINDOW_FULLSCREEN_DESKTOP))) ? 1 : 0;
	Uint32 DesiredFullscreenMode = GetFullscreenMode(Client->UseDesktopResolution);
	UBOOL IsDesiredFullscreenMode = (Window && (SDL_GetWindowFlags(Window) & (SDL_WINDOW_FULLSCREEN | SDL_WINDOW_FULLSCREEN_DESKTOP)) == DesiredFullscreenMode) ? 1 : 0;
	UBOOL ShouldResize = 1;


	debugf(TEXT("USDLViewport::ResizeViewport(%d, %d) - RequestFullscreen %d - IsFullscreen %d - DesiredFullscreenMode %d - IsDesiredFullscreenMode %d"), NewX, NewY, RequestFullscreen, IsFullscreen, DesiredFullscreenMode, IsDesiredFullscreenMode);
	
	// Align NewX.
	check(NewX>=0);
	check(NewY>=0);
//	NewX = Align(NewX,2);

	if( !(NewBlitFlags & BLIT_Temporary) )
	{
		ScreenPointer = NULL;
	}

	// Set new info.
	DWORD OldBlitFlags = BlitFlags;
	BlitFlags          = NewBlitFlags & ~BLIT_ParameterFlags;
	SizeX              = NewX;
	SizeY              = NewY;

	Uint32 VideoFlags = SDL_WINDOW_ALLOW_HIGHDPI | SDL_WINDOW_RESIZABLE;
	INT VideoBPP   = 0;

#if MACOSX
	// !!! FIXME: Hack. --ryan.
//	VideoBPP = (RenDev->Use16bit) ? 16 : 32;
#endif

	// Pull current driver string.
	FString CurrentDriver = RenDev->GetClass()->GetPathName();

	if ( !appStrcmp( *CurrentDriver, TEXT("SDLSoftDrv.SDLSoftwareRenderDevice") ) )
	{		
		debugf( TEXT("SDLSoftDrv.SDLSoftwareRenderDevice") );
		VideoBPP = 16;
		ColorBytes = 2;
		Caps |= CC_RGB565;
	}

	if( NewBlitFlags & BLIT_OpenGL )
	{
		VideoFlags |= SDL_WINDOW_OPENGL;

		// stijn: force mac to use a 32-bit depth buffer regardless of what the rendev requests
#if MACOSX
		SDL_GL_SetAttribute(SDL_GL_DEPTH_SIZE, 32);
#endif
	}
	else if ( NewBlitFlags & BLIT_Vulkan )
	{
		VideoFlags |= SDL_WINDOW_VULKAN;
	}

	if( NewBlitFlags & BLIT_Fullscreen )
	{
		VideoFlags |= DesiredFullscreenMode;
	}

	if( Client->BorderlessWindow && !RequestFullscreen && !ParseParam(appCmdLine(),TEXT("window")) )
	{
		VideoFlags |= SDL_WINDOW_BORDERLESS;
	}

	SDL_ShowCursor(SDL_ENABLE);
	if (!Window)
	{
		if ( (Window = SDL_CreateWindow( "", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, NewX, NewY, VideoFlags )) == NULL )
		{
			appErrorf( TEXT("Couldn't create SDL window: %s\n"), appFromAnsi(SDL_GetError()) );
			appExit();
		}

		IsFullscreen = (SDL_GetWindowFlags(Window) & DesiredFullscreenMode) ? 1 : 0;

		if (IsFullscreen != RequestFullscreen) 
		{
			appErrorf( TEXT("Inconsistent SDL window flags\n"));
			appExit();
		}

	}	
	else
	{
		SDL_DisplayMode Mode{};
		if (SDL_GetWindowDisplayMode(0, &Mode) == -1)
			Mode.format = SDL_PIXELFORMAT_UNKNOWN;//BGRA8888;

		// stijn: check if we actually need to resize. Chances are we came here
		// after receiving an SDL_WINDOWEVENT_RESIZED
		if (IsFullscreen && IsDesiredFullscreenMode && RequestFullscreen && Mode.w == NewX && Mode.h == NewY)
		{
			ShouldResize = 0;
		}
		else if (RequestFullscreen && DesiredFullscreenMode == SDL_WINDOW_FULLSCREEN_DESKTOP)
		{
			ShouldResize = 0;
		}
		else if (!IsFullscreen && !RequestFullscreen)
		{
			INT WindowPixelSizeX, WindowPixelSizeY;
            if (NewBlitFlags & (BLIT_OpenGL|BLIT_Vulkan))
                SDL_GL_GetDrawableSize(Window, &WindowPixelSizeX, &WindowPixelSizeY);
            else
                SDL_Metal_GetDrawableSize(Window, &WindowPixelSizeX, &WindowPixelSizeY);

			if (WindowPixelSizeX == NewX && WindowPixelSizeY == NewY)
				ShouldResize = 0;
		}

		Mode.w = NewX;
		Mode.h = NewY;

		if (RequestFullscreen)
		{
			// switch to fullscreen mode if we're not there already
			if ((!IsFullscreen || !IsDesiredFullscreenMode) && !InResize && SDL_SetWindowFullscreen(Window, VideoFlags & DesiredFullscreenMode))
			{
				appErrorf( TEXT("Couldn't set SDL window fullscreen state: %s\n"), appFromAnsi(SDL_GetError()) );
				appExit();
			}

			IsFullscreen = SDL_GetWindowFlags(Window) & DesiredFullscreenMode;

			if (ShouldResize && !InResize)
			{
				InResize = 1;
				if (SDL_SetWindowDisplayMode(Window, &Mode))
				{
					appErrorf( TEXT("Couldn't set SDL window display mode: %s\n"), appFromAnsi(SDL_GetError()) );
					appExit();
				}
				SDL_SetWindowSize(Window, NewX, NewY);
				InResize = 0;
			}
		}
		else
		{
			// we want windowed mode
			if (IsFullscreen)
			{
				if (SDL_SetWindowFullscreen(Window, 0))
				{
					appErrorf( TEXT("Couldn't set SDL window fullscreen state: %s\n"), appFromAnsi(SDL_GetError()) );
					appExit();
				}
				
				IsFullscreen = SDL_GetWindowFlags(Window) & DesiredFullscreenMode;
			}

			if (ShouldResize && !InResize)
			{
				InResize = 1;
				// stijn: Necessary because setting the size of a maximized window
				// will not update the window frame otherwise
				SDL_RestoreWindow(Window);
				
				// now set the size
				SDL_SetWindowSize(Window, NewX, NewY);
				InResize = 0;
			}
		}	
		
	}

	if (!Window)
	{
		appErrorf(TEXT("Could not create SDL Window (%dx%d): %ls\n"), NewX, NewY, appFromAnsi(SDL_GetError()));
		return 0;
	}

	// Get effective window size. Might differ from what we requested
	INT WindowPointSizeX, WindowPointSizeY, WindowPixelSizeX, WindowPixelSizeY;
	SDL_GetWindowSize(Window, &WindowPointSizeX, &WindowPointSizeY);
	if (BlitFlags & (BLIT_OpenGL|BLIT_Vulkan))
	{
		SDL_GL_GetDrawableSize(Window, &WindowPixelSizeX, &WindowPixelSizeY);
	}
	else
	{
		WindowPixelSizeX = WindowPointSizeX;
		WindowPixelSizeY = WindowPointSizeY;
	}
	
	MouseScaleX = WindowPointSizeX>0 ? static_cast<DOUBLE>(WindowPixelSizeX) / WindowPointSizeX : 1.0;
	MouseScaleY = WindowPointSizeY>0 ? static_cast<DOUBLE>(WindowPixelSizeY) / WindowPointSizeY : 1.0;
	debugf(TEXT("SDLDrv: Window Point Size %dx%d - Window Pixel Size %dx%d - Mouse Scale %lf,%lf"), WindowPointSizeX, WindowPointSizeY, WindowPixelSizeX, WindowPixelSizeY, MouseScaleX, MouseScaleY);
	
	if (WindowPointSizeX != NewX || WindowPointSizeY != NewY)
	{
		debugf(TEXT("SDLDrv WARNING: Requested %dx%d window but got %dx%d - this is probably a high-dpi scaled resolution"), NewX, NewY, WindowPointSizeX, WindowPointSizeY);
	}

	if (WindowPointSizeX > 0 &&
		WindowPointSizeY > 0 &&
		RenDev &&
		!GIsEditor &&
		bSaveSize &&
		!ParseParam(appCmdLine(),TEXT("window")))
	{
		if( IsFullscreen && !Client->UseDesktopResolution )
		{
			Client->FullscreenViewportX = WindowPointSizeX;
			Client->FullscreenViewportY = WindowPointSizeY;
			Client->FullscreenColorBits = ColorBytes*8;
			Client->SaveConfig();
		}
		else if( !IsFullscreen )
		{
			Client->WindowedViewportX = WindowPointSizeX;
			Client->WindowedViewportY = WindowPointSizeY;
			Client->WindowedColorBits = ColorBytes*8;
			Client->SaveConfig();
		}
	}

	SizeX = WindowPixelSizeX;
	SizeY = WindowPixelSizeY;
	//SizeX = NewX * 2;
	//SizeY = NewY * 2;
	// Just grab once at startup; browsers let you ungrab with Escape,
	//  and SDL will regrab when the user clicks the canvas again.
#ifdef __EMSCRIPTEN__  // we grab this elsewhere for Emscripten.
	SDL_SetHint(SDL_HINT_MOUSE_RELATIVE_MODE_WARP, 0);
	SDL_SetHint(SDL_HINT_MOUSE_RELATIVE_SCALING, 0);

	SDL_SetRelativeMouseMode(SDL_TRUE);
#endif

    SetTitleBar();

	// Make this viewport current and update its title bar.
	GetOuterUClient()->MakeCurrent( this );

	// Update audio.
	if( SavedViewport && SavedViewport!=Client->Engine->Audio->GetViewport() )
		Client->Engine->Audio->SetViewport( SavedViewport );

	// Update the window.
	UpdateWindowFrame();
	
	UpdateMouseGrabState(Client->CaptureMouse && MouseIsGrabbed);

	return 1;
	unguard;
}

//
// Get localized keyname.
//
TCHAR * USDLViewport::GetLocalizedKeyName( EInputKey Key )
{
	static TCHAR retval[] = { 0 };
	return retval;  // !!! FIXME: implement this! --ryan.
}


void USDLViewport::UpdateSpeech()
{
    #if MACOSX
    if (MacTextToSpeechEnabled)
    {
        if (SpeechQueue.Num())
        {
            if (!SpeechBusy())
            {
                FString &str = SpeechQueue(0);
                INT len = str.Len();
                char *cvt = (char *) TCHAR_TO_ANSI(*str);
                if (cvt)
                {
                    int rc = (int) SpeakText(MacTextToSpeechChannel, cvt, len);
                    if (rc != 0)
                        debugf(TEXT("TTS: Failed to speak text! rc==%d"), rc);
                }
                SpeechQueue.Remove(0);
            }
        }
    }
    #endif
}


void USDLViewport::TextToSpeech( const FString& Text, FLOAT Volume )
{
    INT len = Text.Len();
    if (len == 0)
        return;

    #if MACOSX
    if (MacTextToSpeechEnabled)
    {
        if (SpeechQueue.Num() < 150)  // in case of irc flood...
			new(SpeechQueue) FString(Text);
    }
    #endif

    if (TextToSpeechObject != -1)
    {
        char *cvt = (char *) TCHAR_TO_ANSI(*Text);
        if (cvt)
        {
            // !!! FIXME: make non-blocking and use SpeechQueue
            if (write(TextToSpeechObject, cvt, len) == -1 || 
				cvt[len-1] != '\n' ||
				write(TextToSpeechObject, "\n", 1) == -1)
			{
				debugf(TEXT("Could not write to TextToSpeechObject"));
			}
        }
    }

    UpdateSpeech();
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

