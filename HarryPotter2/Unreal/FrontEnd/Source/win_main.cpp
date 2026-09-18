// -----------------------------------------------------------------------------
//           _                              _                            
//          (_)                            (_)                           
// __      ___ _ __         _ __ ___   __ _ _ _ __       ___ _ __  _ __  
// \ \ /\ / / | '_ \       | '_ ` _ \ / _` | | '_ \     / __| '_ \| '_ \ 
//  \ V  V /| | | | |      | | | | | | (_| | | | | | _ | (__| |_) | |_) |
//   \_/\_/ |_|_| |_|      |_| |_| |_|\__,_|_|_| |_|(_) \___| .__/| .__/ 
//                   ______                                 | |   | |    
//                  |______|                                |_|   |_|    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#define WIN32_LEAN_AND_MEAN
#include <windows.h>				

#include ".\common\types.h"

#include ".\MenuManager\MenuManager.h"
#include ".\FrontEnd.h"

#include ".\resource.h"

// ***************
// *** GLOBALS ***
// ***************
CFrontEnd	g_FrontEnd;
bool		g_bAppActive = true;

// ******************
// *** PROTOTYPES ***
// ******************
INT_PTR CALLBACK	MainMenu_DialogProc			( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );

//**********************************************************************************
//	FUNCTION NAME:	WinMain
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	7/12/2002
//	
//	ARGUMENT 1:		HINSTANCE hInstance		- Handle to the current instance of the application
//	ARGUMENT 2:		HINSTANCE hPrevInstance	- Handle to the previous instance of the application
//	ARGUMENT 3:		LPSTR lpCmdLine			- String that has the command line for our app, excluding the program name
//	ARGUMENT 4:		int nCmdShow			- Specifies how the window is to be shown
//	RETURN TYPE:	int APIENTRY 			- Zero means exited before WndProc, else the wParam in WM_QUIT message == success/failure.
//	DESCRIPTION:	Initialize the application, display its main window, and enter a message retrieval-and-dispatch system... return when done.
//**********************************************************************************
int APIENTRY WinMain( HINSTANCE hInstance,
                      HINSTANCE hPrevInstance,
                      LPSTR     lpCmdLine,
                      int       nCmdShow )
{
	// Init our launcher class
	if( FAILED( g_FrontEnd.Init( hInstance, lpCmdLine )) )
		return 0;
	
	//*************************
	//**** MAIN WHILE LOOP ****
	//*************************
	MSG		msg;
	while(1)
	{
		if( PeekMessage(&msg, NULL, 0, 0, PM_REMOVE) )
		{
			if( msg.message == WM_QUIT )
				break; // break out of our while loop
			TranslateMessage(&msg);
			DispatchMessage(&msg);
		}
		else if( g_bAppActive )//we are in focus (app is active)
		{
			// Update our menu manager
			g_FrontEnd.Update();
		}
		else //we are not in focus (app is not active)
		{
			WaitMessage();	//so we don't suck up cpu time when the appication is not active
		}
	}
	
	// Clean up
	g_FrontEnd.Destroy();

	return msg.wParam;
}
