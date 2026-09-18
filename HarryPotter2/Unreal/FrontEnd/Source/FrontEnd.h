// -----------------------------------------------------------------------------
//  ______                  _   ______           _     _     
// |  ____|                | | |  ____|         | |   | |    
// | |__   _ __  ___  _ __ | |_| |__   _ __   __| |   | |__  
// |  __| | '__|/ _ \| '_ \| __|  __| | '_ \ / _` |   | '_ \ 
// | |    | |  | (_) | | | | |_| |____| | | | (_| | _ | | | |
// |_|    |_|   \___/|_| |_|\__|______|_| |_|\__,_|(_)|_| |_|
//                                                           
//    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#ifndef	_H_FRONTEND_
#define _H_FRONTEND_

#include ".\Bitmap\Bitmap.h"

// ***************
// *** DEFINES ***
// ***************

// Directory info
#define DEFAULT_GAME_DIR						("\\Harry Potter and the Chamber of Secrets")
#define DEFAULT_SAVE_DIR						("\\Save")
#define DEFAULT_SAVE_FILENAME					("Save0.usa")
#define DEFAULT_THUMBNAIL_FILENAME				("Save0.bmp")
#define DEFAULT_LAUNCHAPP_FILENAME				("game.exe")
												
// Command line tokens							
#define NEWGAME_COMMAND_LINE_TOKEN				("PrivetDr.unr")
#define LOADGAME_COMMAND_LINE_TOKEN				("-LOAD=0")
												
// Bitmap filenames
#define BITMAP_PATH								("..\\Help")

#define BITMAP_FILENAME_SPLASH_0				("EALogo1.bmp")
#define BITMAP_FILENAME_SPLASH_1				("WBLegal.bmp")
												
#define BITMAP_FILENAME_MAINMENU_BACKGROUND		("MainMenu.bmp")
#define BITMAP_FILENAME_NEWGAME_BACKGROUND		("MainMenu.bmp")
#define BITMAP_FILENAME_LOADGAME_BACKGROUND		("MainMenu.bmp")
#define BITMAP_FILENAME_CONFIG_BACKGROUND		("MainMenu.bmp")
												
#define BITMAP_FILENAME_BUTTON_UP				("ButtonUp.bmp")
#define BITMAP_FILENAME_BUTTON_DOWN				("ButtonDown.bmp")
#define BITMAP_FILENAME_BUTTON_DISABLED			("ButtonDisabled.bmp")

#define BITMAP_FILENAME_BUTTON_THUMB_UP			("ButtonThumbUp.bmp")
#define BITMAP_FILENAME_BUTTON_THUMB_DOWN		("ButtonThumbDown.bmp")
#define BITMAP_FILENAME_BUTTON_THUMB_DISABLED	("ButtonThumbDisabled.bmp")

// Splash config
#define SPLASH_WAIT_TIME				1500
#define SPLASH_MAX_SCREENS				4

#define NUM_SAVE_SLOTS					6


struct SaveSlotData
{
	char	m_Path[1024];	// the file path for the save game
	CBitmap m_bmpThumnail;	// thumbnail for this slot
	bool	m_bEmpty;		// is this slot empty?

	SaveSlotData() : m_bEmpty(true) { m_Path[0]=0; }
};

struct IMenuManager;
struct IMenu;

//**********************************************************************************
//	CLASS NAME:		CFrontEnd	
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	7/12/2001
//
//	DESCRIPTION:	This singleton class will encapsilate all FrontEnd related functionality
//**********************************************************************************
class CFrontEnd
{
private:
	// *** Private Data	
	HWND			m_hWnd;							// handle to current menu
	HINSTANCE		m_hInstance;					// handle to our module's instance
	
	SaveSlotData	m_SaveSlots[NUM_SAVE_SLOTS];	// array of SaveSlotData
	char			m_strCommandLine[1024];			// command line string
	
	IMenuManager*	m_pMenuManager;					// Menu Manager interface
	IMenu*			m_pCurMenu;						// Current Menu
	
	// --- Button bitmap handles
	CBitmap			m_bmpBackground;				// background bitmap used by all menus for their background
	CBitmap			m_bmpButtonUp;					// button up bitmap
	CBitmap			m_bmpButtonDown;				// button down bitmap
	CBitmap			m_bmpButtonDisabled;			// button disabled bitmap
	
	CBitmap			m_bmpButtonThumbUp;				// button disabled bitmap
	CBitmap			m_bmpButtonThumbDown;			// button down bitmap
	CBitmap			m_bmpButtonThumbDisabled;		// button down bitmap
	
	// --- Splash Menu specific
	int				m_iSplashIndex;					// current splash screen index
	char			m_strSplashQueue[SPLASH_MAX_SCREENS][512];	// 
	
	// --- GameSettings
	bool			m_bGameNoSound;					//"-NOSOUND"
	bool			m_bGameWindowed;				//"-WINDOW"
	bool			m_bGameForceSoftware;			//"-ForceSoftware"
	
	//"-3Dsetup"
	//"testrendev="
	//"testrendev=D3DDrv.D3DRenderDevice log=Detected.log"

	// *** Private Functions
	void	Clear								( void );
	HRESULT LaunchGame							( const u32 iSlot );
	char*	GetBitmapPath						( char* strFilename );

public:
	// *** Public Functions
	
	// --- Singleton Pattern Functions
	static inline CFrontEnd& Instance			( void ) { static CFrontEnd Obj; return Obj; }	
			CFrontEnd							( void ) { Clear();   }
			~CFrontEnd							( void ) { Destroy(); }
	
	HRESULT	Init								( const HINSTANCE hInstance, const char* strCommandLine );
	void	Destroy								( void );
	void	Update								( void );
	
	// --- Launch Functions
	HRESULT LaunchLoadGame						( const u32 iSlot );
	HRESULT LaunchNewGame						( const u32 iSlot );
	
	// --- Save Slot File Managment Functions
	u32		GetSlotIndexFromResourceID			( const u32 iID   );
	u32		GetResourceIDFromSlotIndex			( const u32 iSlot );
	void	EmptySaveSlotFiles					( const u32 iSlot );
	
	// *** Menu Functions ***
	void	PopCurrentMenu						( void );
	void	PaintButton							( HWND hWndDlg, int iDlgItemID );
	void	OnDrawItemButton					( HWND hWndDlg, LPDRAWITEMSTRUCT pDIS );
	void	OnInitDialog						( HWND hWndDlg );

	// --- Splash Menu Functions
	HRESULT Splash_OnOpen						( const HWND hWndDlg );
	void	Splash_OnClose						( void );
	void	Splash_OnPaint						( const HWND hWndDlg );
	void	Splash_OnTimer						( const HWND hWndDlg );
	
	// --- Main Menu Functions
	HRESULT MainMenu_OnOpen						( const HWND hWndDlg );
	void	MainMenu_OnClose					( void );
	void	MainMenu_OnPaint					( const HWND hWndDlg );
	void	MainMenu_OnButtonNewGame			( void );
	void	MainMenu_OnButtonLoadGame			( void );
	void	MainMenu_OnButtonConfig				( void );
	
	// --- New Game Menu Functions
	HRESULT NewGameMenu_OnOpen					( const HWND hWndDlg );
	void	NewGameMenu_OnClose					( void );
	void	NewGameMenu_OnPaint					( const HWND hWndDlg );
	void	NewGameMenu_OnButtonSlot			( const u32 iSlot );
	void	NewGameMenu_OnDrawItemButtonSlot	( HWND hWndDlg, LPDRAWITEMSTRUCT pDIS );
	
	// --- Load Game Menu Functions
	HRESULT LoadGameMenu_OnOpen					( const HWND hWndDlg );
	void	LoadGameMenu_OnClose				( void );
	void	LoadGameMenu_OnPaint				( const HWND hWndDlg );
	void	LoadGameMenu_OnButtonSlot			( const u32 iSlot );
	void	LoadGameMenu_OnDrawItemButtonSlot	( HWND hWndDlg, LPDRAWITEMSTRUCT pDIS );
	
	// --- Configure Menu Functions
	HRESULT ConfigMenu_OnOpen					( const HWND hWndDlg );
	void	ConfigMenu_OnClose					( void );
	void	ConfigMenu_OnPaint					( const HWND hWndDlg );
	void	ConfigMenu_OnButtonVideoConfig		( void );
	void	ConfigMenu_OnButtonJoystickConfig	( void );
	
};

extern CFrontEnd g_FrontEnd;

#endif // _H_FRONTEND_
// --------------------------------------------------------------------------------
// FRONTEND.h - End of file
// --------------------------------------------------------------------------------