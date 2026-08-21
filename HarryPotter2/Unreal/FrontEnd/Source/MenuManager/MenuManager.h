// -----------------------------------------------------------------------------
//  __  __                  __  __                                       _     
// |  \/  |                |  \/  |                                     | |    
// | \  / | ___ _ __  _   _| \  / | __ _ _ __   __ _  __ _  ___ _ __    | |__  
// | |\/| |/ _ \ '_ \| | | | |\/| |/ _` | '_ \ / _` |/ _` |/ _ \ '__|   | '_ \ 
// | |  | |  __/ | | | |_| | |  | | (_| | | | | (_| | (_| |  __/ |    _ | | | |
// |_|  |_|\___|_| |_|\__,_|_|  |_|\__,_|_| |_|\__,_|\__, |\___|_|   (_)|_| |_|
//                                                    __/ |                    
//                                                   |___/                     
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002 by Elijah Emerson
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#ifndef	_H_MENU_MANAGER_
#define _H_MENU_MANAGER_

struct IMenuManager;
struct IMenu;

// *** Call this function to create the MenuManager interface
IMenuManager*	CreateMenuManager( HINSTANCE hInstance );


// Windows messages for initilization of a menu as well as when a menu is destoried
#define WM_MM_OPEN		WM_USER+1
#define WM_MM_CLOSE		WM_USER+2


// ---------------------------------------------------------------------------------
//	INTERFACE NAME:	 IMenu
//	AUTHOR(S):		 Elijah Emerson
//	CREATION DATE:	 07/21/2002
//
//	DESCRIPTION:	 IMenu is an interface for a single menu.
// ---------------------------------------------------------------------------------
struct __declspec(novtable) IMenu
{
	// *** Interface Functions ***
	// --- Base Functions
	virtual	HRESULT			Release				( void ) = 0;
	
	// --- Set Functions
	virtual void			SetPopTimer			( f32 fWaitTime ) = 0;
	virtual void			SetRelativePosition	( f32 fXPercent, f32 fYPercent ) = 0;
	
	// --- Get Functions
	virtual HWND			GetHandle			( void ) const = 0;
};


// -----------------------------------------------------------------------------
//	INTERFACE NAME:	IMenuManager
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	7/12/2001
//
//	DESCRIPTION:	IRenderer is the interface to the renderer
// -----------------------------------------------------------------------------
struct __declspec(novtable) IMenuManager
{
	// *** Interface Implementation ***
	// --- Base Functions
	virtual HRESULT			Release				( void ) = 0;
	virtual void			Update				( void ) = 0;
	
	// --- Menu Management
	virtual IMenu*			PushMenu			( const char* strName, const LPCTSTR lpTemplate, const DLGPROC dlgProc ) = 0;
	virtual IMenu*			PopMenu				( void ) = 0;
	
	// --- Query Functions
	virtual bool			IsActive			( void ) const = 0;
	virtual bool			IsMinimized			( void ) const = 0;
	virtual u32				GetNumMenus			( void ) = 0;
	virtual HWND			GetLastMenuHandle	( void ) = 0;
	virtual HWND			GetCurrentMenuHandle( void ) = 0;
};

#endif // _H_MENU_MANAGER_
// --------------------------------------------------------------------------------
// FRONTEND.h - End of file
// --------------------------------------------------------------------------------