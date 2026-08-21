// -----------------------------------------------------------------------------
//  ______ ______        __  __       _       __  __                                       
// |  ____|  ____|      |  \/  |     (_)     |  \/  |                                      
// | |__  | |__         | \  / | __ _ _ _ __ | \  / | ___ _ __  _   _      ___ _ __  _ __  
// |  __| |  __|        | |\/| |/ _` | | '_ \| |\/| |/ _ \ '_ \| | | |    / __| '_ \| '_ \ 
// | |    | |____       | |  | | (_| | | | | | |  | |  __/ | | | |_| | _ | (__| |_) | |_) |
// |_|    |______|      |_|  |_|\__,_|_|_| |_|_|  |_|\___|_| |_|\__,_|(_) \___| .__/| .__/ 
//                ______                                                      | |   | |    
//               |______|                                                     |_|   |_|    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#define WIN32_LEAN_AND_MEAN
#include <windows.h>				
#include <stdio.h>					

#include ".\common\types.h"			
#include ".\common\win_utilities.h"	

#include ".\MenuManager\MenuManager.h"

#include ".\FrontEnd.h"				
#include ".\resource.h"				


// ******************
// *** PROTOTYPES ***
// ******************
INT_PTR CALLBACK	MainMenu_DialogProc			( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );
INT_PTR CALLBACK	NewGameMenu_DialogProc		( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );
INT_PTR CALLBACK	LoadGameMenu_DialogProc		( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );
INT_PTR CALLBACK	ConfigMenu_DialogProc		( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );

// *************************************************************************************************************
// *************************************************************************************************************
// -- Main Menu Functions

HRESULT CFrontEnd::MainMenu_OnOpen( const HWND hWndDlg )
{
	if( hWndDlg == NULL ) return E_INVALIDARG;

	// Center the dialog box to the screen
	SetWindowText( hWndDlg, ("FrontEnd: Main Menu") );
	
	// destroy the curent splash screen and load the new one
	m_bmpBackground.Destroy();
	if( !m_bmpBackground.LoadBmpFile( GetBitmapPath(BITMAP_FILENAME_MAINMENU_BACKGROUND) ) )
		return E_FAIL;
	
	Util_SetWindow( hWndDlg, 0.5f, 0.5f, m_bmpBackground.GetWidth(), m_bmpBackground.GetHeight() );
	
//	SendDlgItemMessage( hWndDlg, IDC_BUTTON_NEWGAME,	BM_SETIMAGE, IMAGE_BITMAP, (LPARAM)m_hbmpButtonUp	 );
	return S_OK;
}

void CFrontEnd::MainMenu_OnButtonNewGame( void )
{
	// --- Push our New Game Menu onto the stack
	m_pMenuManager->PushMenu( "NewGame", MAKEINTRESOURCE(IDD_DIALOG_SLOTMENU), NewGameMenu_DialogProc );
}

void CFrontEnd::MainMenu_OnButtonLoadGame( void )
{
	// --- Push our Load Game Menu onto the stack
	m_pMenuManager->PushMenu( "LoadGame", MAKEINTRESOURCE(IDD_DIALOG_SLOTMENU), LoadGameMenu_DialogProc );
}

void CFrontEnd::MainMenu_OnButtonConfig( void )
{	
	// --- Push our Load Game Menu onto the stack
	m_pMenuManager->PushMenu( "Configure", MAKEINTRESOURCE(IDD_DIALOG_CONFIGMENU), ConfigMenu_DialogProc );
}

void CFrontEnd::MainMenu_OnPaint( const HWND hWndDlg )
{
	RECT rc;
	if( GetUpdateRect(hWndDlg, &rc, false ) )
	{
		HDC hDC = GetDC( hWndDlg );
		m_bmpBackground.Draw( hDC, rc.left, rc.top, rc.left, rc.top, rc.right-rc.left, rc.bottom-rc.top );
		ReleaseDC( hWndDlg, hDC );
	}
}


INT_PTR CALLBACK MainMenu_DialogProc( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam )
{
	switch( iMsg )
	{
		// ************* Windows Messages for Menu Manager ***************
		case WM_MM_OPEN:
			g_FrontEnd.MainMenu_OnOpen( hWndDlg );
			return 1;
		
		case WM_MM_CLOSE:
			return 1;
		// ***************************************************************
		
		case WM_PAINT:
			g_FrontEnd.MainMenu_OnPaint( hWndDlg );
			break;
		
		case WM_DRAWITEM:
			g_FrontEnd.OnDrawItemButton( hWndDlg, (LPDRAWITEMSTRUCT)lParam );
			break;
		
		case WM_KILLFOCUS:
			if( hWndDlg == GetParent( (HWND)wParam ) )
				SetFocus( hWndDlg );
			break;
		
		case WM_INITDIALOG:
			g_FrontEnd.OnInitDialog( hWndDlg );
			return true;
		
		case WM_NCCALCSIZE:  //CAPTURE THIS MESSAGE AND RETURN NULL
			break;
		
		case WM_COMMAND:
			switch( LOWORD(wParam) )
			{
				case IDC_BUTTON_NEWGAME:		g_FrontEnd.MainMenu_OnButtonNewGame( );			break;
				case IDC_BUTTON_LOADGAME:		g_FrontEnd.MainMenu_OnButtonLoadGame( );			break;
				case IDC_BUTTON_CONFIG:			g_FrontEnd.MainMenu_OnButtonConfig( );			break;
				case IDC_BUTTON_EXIT:			g_FrontEnd.PopCurrentMenu( );					break;
			}
			break;
		
		case WM_CLOSE:		
			g_FrontEnd.PopCurrentMenu();
			return 0;
		
		case WM_DESTROY:
            PostQuitMessage( 0 );
            return 0;
	}
	return false;
}

