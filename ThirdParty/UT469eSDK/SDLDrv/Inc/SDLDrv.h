/*=============================================================================
	SDLDrv.h: Simple Directmedia Layer cross-platform driver.
	Copyright 2002 Epic Games, Inc. All Rights Reserved.

        SDL website: http://www.libsdl.org/

Revision history:
	* Created by Ryan C. Gordon, based on WinDrv.
      This is an updated rewrite of the original SDLDrv.
=============================================================================*/

#ifndef _INC_SDLDRV
#define _INC_SDLDRV

#if MACOSX
//#include <SpeechSynthesis.h>
// OSX system headers conflict with several Unreal symbols (FVector, verify,
//  etc...), so hardcode the predeclarations we need here...)  --ryan.
extern "C" {
    struct SpeechChannelRecord {
      long                data[1];
    };
    typedef struct SpeechChannelRecord      SpeechChannelRecord;
    typedef SpeechChannelRecord *           SpeechChannel;

    typedef signed short OSErr;
    OSErr NewSpeechChannel(void *dummy, SpeechChannel *chan);
    OSErr SpeakText(SpeechChannel chan, const void *buf, unsigned long len);
    OSErr StopSpeech(SpeechChannel chan);
    short SpeechBusy(void);
    OSErr DisposeSpeechChannel(SpeechChannel chan);
}
#endif

/*----------------------------------------------------------------------------
	API.
----------------------------------------------------------------------------*/

#ifndef SDLDRV_API
	#define SDLDRV_API DLL_IMPORT
#endif

/*----------------------------------------------------------------------------
	Dependencies.
----------------------------------------------------------------------------*/

#include <SDL2/SDL.h>

// Unreal includes.
#include "Engine.h"
#include "UnRender.h"

/*-----------------------------------------------------------------------------
	Localization Helpers
-----------------------------------------------------------------------------*/
inline FString LocalizeGeneralHelper(const ANSICHAR* Key, const TCHAR* Package)
{
#if ENGINE_VERSION==227
	return LocalizeGeneral(appFromAnsi(Key), Package);
#else
	return LocalizeGeneral(Key, Package);
#endif
}

inline FString LocalizeGeneralHelper(const TCHAR* Key, const TCHAR* Package)
{
	return LocalizeGeneral(Key, Package);
}

/*-----------------------------------------------------------------------------
	Declarations.
-----------------------------------------------------------------------------*/

// Classes.
class USDLViewport;
class USDLClient;

/*-----------------------------------------------------------------------------
	USDLClient.
-----------------------------------------------------------------------------*/

//
// SDL implementation of the client.
//
class USDLClient : public UClient, public FNotifyHook
{
	DECLARE_CLASS(USDLClient,UClient,CLASS_Transient|CLASS_Config,SDLDrv)

	// Configuration.
	BITFIELD			UseJoystick;
	BITFIELD			StartupFullscreen;
	BITFIELD            BorderlessWindow;
	BITFIELD            UseDesktopResolution;
	INT                 JoystickNumber;
	INT                 JoystickHatNumber;
	FLOAT               ScaleJBX;
	FLOAT               ScaleJBY;
	FLOAT               JoystickDeadZone;
	UBOOL               IgnoreHat;
	UBOOL               IgnoreUngrabbedMouse;
	UBOOL               AllowUnicodeKeys;
	UBOOL               AllowCommandQKeys;
	UBOOL               MacFakeMouseButtons;
	UBOOL               MacKeepAllScreensOn;
	UBOOL               MacNativeTextToSpeech;
	FStringNoInit       TextToSpeechFile;
	FLOAT               UIScale;

	// Variables.
	UBOOL				InMenuLoop;
	UBOOL				ConfigReturnFullscreen;
	INT					NormalMouseInfo[3];
	INT					CaptureMouseInfo[3];

	enum { MaxJoystickAxes = 8 };

    static SDL_Joystick *Joystick;
    static SDL_JoystickID JoystickInstanceId;
    static int JoystickAxes;
    static int JoystickButtons;
    static int JoystickHats;
    static int JoystickBalls;

	// Constructors.
	USDLClient();
	void StaticConstructor();
	UBOOL OpenJoystick();
	void CloseJoystick();

	// FNotifyHook interface.
	void NotifyDestroy( void* Src );

	// UObject interface.
	void Destroy();
	void PostEditChange();
	void ShutdownAfterError();

	// UClient interface.
	void Init( UEngine* InEngine );
	void ShowViewportWindows( DWORD ShowFlags, INT DoShow );
	void EnableViewportWindows( DWORD ShowFlags, INT DoEnable );
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar=*GLog );
	void Tick();
	void MakeCurrent( UViewport* InViewport );
	UViewport* GetLastCurrent();
	class UViewport* NewViewport( const FName Name );
    void TeardownSR();
	FLOAT GetUIScale() const { return UIScale; }

	FString GetClipboardText();
	UBOOL SetClipboardText(FString& Str);

	void StartTextInput();
	void StopTextInput();

	// Dynamic font creation support
	INT GetDPIScaledX(INT X);
	INT GetDPIScaledY(INT Y);
#if UNREAL_TOURNAMENT_OLDUNREAL
	void* CreateTTFFont(const TCHAR* FontName, int Height, int Italic, int Bold, int Underlined, int AntiAliased);
	void MakeGlyphsList(FBitmask& RequestedGlyphs, void* InFont, TArray<FGlyphInfo>& OutGlyphs, int AntiAlias);
	void RenderGlyph(FGlyphInfo& Info, UTexture* PageTexture, INT X, INT Y, INT YAdjust);
	void DestroyGlyphsList(TArray<FGlyphInfo>& Glyphs);
	void DestroyTTFFont(void* Font);
#endif
};

/*-----------------------------------------------------------------------------
	USDLViewport.
-----------------------------------------------------------------------------*/

//
// An SDL viewport.
//
class USDLViewport : public UViewport
{
	DECLARE_CLASS(USDLViewport,UViewport,CLASS_Transient,SDLDrv)
	DECLARE_WITHIN(USDLClient)

	// Variables.
    SDL_Window *		Window;
	DWORD				BlitFlags;

    UBOOL				LostGrab;
    UBOOL				LostFullscreen;

	// SDL Keysym to EInputKey map.
	TMap<SDL_Keycode,BYTE> KeysymMap;

	// Joystick Hat Hack
	EInputKey 			LastJoyHat;
	UBOOL				LastKey;

	UBOOL				MouseIsGrabbed;
	INT					TextToSpeechObject;  // file descriptor for named pipe.
	DOUBLE              MouseScaleX; // Scaled coords
	DOUBLE              MouseScaleY; // Scaled coords
	
	#if MACOSX
    UBOOL				MacTextToSpeechEnabled;
    SpeechChannel		MacTextToSpeechChannel;
    TArray<FString> SpeechQueue;
	#endif

	FTime LastUpdateTime;

	// Constructor.
	USDLViewport();

	// UObject interface.
	void Destroy();
	void ShutdownAfterError();

	// UViewport interface.
	UBOOL Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize);
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar );
	UBOOL ResizeViewport( DWORD BlitFlags, INT NewX=INDEX_NONE, INT NewY=INDEX_NONE, UBOOL bSaveSize=true );
	UBOOL IsFullscreen();
	void Unlock( UBOOL Blit );
	void Repaint( UBOOL Blit );
	void SetModeCursor();
	void UpdateWindowFrame();
	void OpenWindow( DWORD ParentWindow, UBOOL Temporary, INT NewX, INT NewY, INT OpenX, INT OpenY );
	void CloseWindow();
	void UpdateInput( UBOOL Reset );
	void* GetWindow();
	void SetMouseCapture( UBOOL Capture, UBOOL Clip, UBOOL FocusOnly );
	void GetMouseState( INT* MouseX, INT* MouseY);
	
	void DrawString( DWORD Flags, UFont* Font, INT& DrawX, INT& DrawY, const TCHAR* Text, const FPlane& Color );
	UFont* CreateNativeFont( const TCHAR* FontName, int Height );
	// USDLViewport interface.
	void UpdateMouseGrabState(const UBOOL bGrab);
	void ToggleFullscreen();
	void EndFullscreen();
	void SetTopness();
	DWORD GetViewportButtonFlags( DWORD wParam );

	UBOOL CauseInputEvent( INT iKey, EInputAction Action, FLOAT Delta=0.0 );
	void ReleaseJoystickInput();
	void ReleaseAllInput();

	virtual void TryRenderDevice( const TCHAR* ClassName, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen );
    void SetTitleBar();

	TCHAR* GetLocalizedKeyName( EInputKey Key );
	void TextToSpeech( const FString& Text, FLOAT Volume );
	void UpdateSpeech();  // call once a frame from UpdateInput().
};


#define AUTO_INITIALIZE_REGISTRANTS_SDLDRV \
	USDLViewport::StaticClass(); \
	USDLClient::StaticClass();

#endif //_INC_SDLDRV
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

