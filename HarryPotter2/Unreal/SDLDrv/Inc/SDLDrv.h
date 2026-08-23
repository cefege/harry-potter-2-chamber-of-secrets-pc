/*=============================================================================
	SDLDrv.h: SDL-based platform driver (client + viewport).
=============================================================================*/

#ifndef _INC_SDLDRV
#define _INC_SDLDRV

/*----------------------------------------------------------------------------
	Dependencies.
----------------------------------------------------------------------------*/

#include "Engine.h"
#include <SDL2/SDL.h>

/*-----------------------------------------------------------------------------
	USDLClient.
-----------------------------------------------------------------------------*/

class USDLViewport;

//
// SDL implementation of the client.
//
class USDLClient : public UClient, public FNotifyHook
{
	DECLARE_CLASS(USDLClient,UClient,CLASS_Transient|CLASS_Config,SDLDrv)

public:

	// Joystick axis budget shared with the input contracts.
	enum { MaxJoystickAxes = 8 };

	// Configuration.
	BITFIELD	UseJoystick;
	BITFIELD	StartupFullscreen;
	BITFIELD	BorderlessWindow;
	BITFIELD	UseDesktopResolution;
	BITFIELD	IgnoreHat;
	BITFIELD	IgnoreUngrabbedMouse;
	BITFIELD	AllowUnicodeKeys;
	BITFIELD	AllowCommandQKeys;
	BITFIELD	MacKeepAllScreensOn;
	BITFIELD	MacNativeTextToSpeech;
	INT			JoystickNumber;
	INT			JoystickHatNumber;
	FLOAT		ScaleJBX, ScaleJBY, JoystickDeadZone;
	FString		TextToSpeechFile;
	FLOAT		UIScale;

	// Variables.
	SDL_Joystick*	Joystick;
	INT				JoyButtons;
	INT				JoyHats;
	INT				JoyAxes;
	UBOOL			JoyHatFoldsToButtons;
	UViewport*		LastCurrent;

	// Constructors.
	USDLClient();
	void StaticConstructor();

	// FNotifyHook interface.
	void NotifyDestroy( void* Src );

	// UObject interface.
	void Destroy();
	void PostEditChange();
	void ShutdownAfterError();

	// UClient interface.
	void Init( UEngine* InEngine );
	void ShowViewportWindows( DWORD ShowFlags, int DoShow );
	void EnableViewportWindows( DWORD ShowFlags, int DoEnable );
	void TeardownSR();
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar=*GLog );
	void Tick();
	void MakeCurrent( UViewport* NewViewport );
	UViewport* GetLastCurrent();
	class UViewport* NewViewport( const FName Name );
	FLOAT GetUIScale() const { return UIScale; }
	FLOAT GetDPIScaledX( FLOAT X ) const { return X * UIScale; }
	FLOAT GetDPIScaledY( FLOAT Y ) const { return Y * UIScale; }

	// USDLClient interface.
	UBOOL OpenJoystick();
	void CloseJoystick();
	FString GetClipboardText() const;
	UBOOL SetClipboardText( const TCHAR* Text );
	void StartTextInput();
	void StopTextInput();
};

/*-----------------------------------------------------------------------------
	USDLViewport.
-----------------------------------------------------------------------------*/

//
// SDL implementation of a viewport.
//
class USDLViewport : public UViewport
{
	DECLARE_CLASS(USDLViewport,UViewport,CLASS_Transient,SDLDrv)
	DECLARE_WITHIN(USDLClient)

public:
	enum { NumSystemCursors = 7 };
	enum { KeysymMapSize = 512 };

	// Variables.
	SDL_Window*		SdlWindow;
	DWORD			BlitFlags;
	BITFIELD		LostGrab:1;
	BITFIELD		LostFullscreen:1;
	BITFIELD		MouseIsGrabbed:1;
	EInputKey		LastJoyHat;
	EInputKey		KeysymMap[KeysymMapSize];
	SDL_Cursor*		SystemCursors[NumSystemCursors];
	FLOAT			MouseScaleX, MouseScaleY;
	INT				SpeechPid;

	// Constructor.
	USDLViewport();

	// UObject interface.
	void Destroy();
	void ShutdownAfterError();

	// UPlayer interface.

	// UViewport interface.
	UBOOL Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData=NULL, INT* HitSize=NULL );
	UBOOL ResizeViewport( DWORD NewBlitFlags, INT NewX=INDEX_NONE, INT NewY=INDEX_NONE, INT NewColorBytes=INDEX_NONE );
	void Unlock( UBOOL Blit );
	void Repaint( UBOOL Blit );
	void SetModeCursor();
	void UpdateWindowFrame();
	void OpenWindow( DWORD ParentWindow, UBOOL Temporary, INT NewX, INT NewY, INT OpenX, INT OpenY );
	void CloseWindow();
	void UpdateInput( UBOOL Reset );
	void* GetWindow();
	void SetMouseCapture( UBOOL Capture, UBOOL Clip, UBOOL FocusOnly=0 );
	void DrawString( DWORD Flags, UFont* Font, INT& DrawX, INT& DrawY, const TCHAR* Text, const FPlane& Color );
	UFont* CreateNativeFont( const TCHAR* FontName, int Height );
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar=*GLog );

	// USDLViewport interface.
	UBOOL IsFullscreen();
	void ToggleFullscreen();
	void EndFullscreen();
	void UpdateMouseGrabState( UBOOL Capture );
	void GetMouseState( FLOAT& X, FLOAT& Y, DWORD& Buttons );
	DWORD GetViewportButtonFlags();
	UBOOL CauseInputEvent( INT iKey, EInputAction Action, FLOAT Delta=0.f );
	void ReleaseJoystickInput();
	void ReleaseAllInput();
	void SetTopness();
	void SetTitleBar( const TCHAR* Title );
	const TCHAR* GetLocalizedKeyName( EInputKey Key ) const;
	void TryRenderDevice( const TCHAR* ClassName, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen );
	void TextToSpeech( const FString& Text, FLOAT Duration );
	void UpdateSpeech();
};

#endif //_INC_SDLDRV

#define AUTO_INITIALIZE_REGISTRANTS_SDLDRV \
    USDLViewport::StaticClass(); \
    USDLClient::StaticClass();
