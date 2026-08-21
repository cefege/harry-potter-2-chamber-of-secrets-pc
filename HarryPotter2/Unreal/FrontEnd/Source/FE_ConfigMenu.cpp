// -----------------------------------------------------------------------------
//  ______ ______         _____              __ _       __  __                                       
// |  ____|  ____|       / ____|            / _(_)     |  \/  |                                      
// | |__  | |__         | |      ___  _ __ | |_ _  __ _| \  / | ___ _ __  _   _      ___ _ __  _ __  
// |  __| |  __|        | |     / _ \| '_ \|  _| |/ _` | |\/| |/ _ \ '_ \| | | |    / __| '_ \| '_ \ 
// | |    | |____       | |____| (_) | | | | | | | (_| | |  | |  __/ | | | |_| | _ | (__| |_) | |_) |
// |_|    |______|       \_____|\___/|_| |_|_| |_|\__, |_|  |_|\___|_| |_|\__,_|(_) \___| .__/| .__/ 
//                ______                           __/ |                                | |   | |    
//               |______|                         |___/                                 |_|   |_|    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#define WIN32_LEAN_AND_MEAN
#include <windows.h>				//
#include <stdio.h>					// sprintf()

#include ".\common\types.h"			//
#include ".\common\win_utilities.h"	//

#include ".\MenuManager\MenuManager.h"

#include ".\FrontEnd.h"				//
#include ".\resource.h"				//


// *************************************************************************************************************
// *************************************************************************************************************
// -- Config Menu Functions

HRESULT CFrontEnd::ConfigMenu_OnOpen( const HWND hWndDlg )
{
	if( hWndDlg == NULL ) return E_INVALIDARG;

	// destroy the curent splash screen and load the new one
//	m_bmpBackground.Destroy();
//	if( !m_bmpBackground.LoadBmpFile( BITMAP_FILENAME_CONFIG_BACKGROUND ) )
//		return E_FAIL;
	
	// Setup the Title of the save slots
	SetWindowText( hWndDlg, ("FrontEnd: Config Menu") );
	SendMessage( GetDlgItem(hWndDlg,IDC_LABEL_ACTION),		WM_SETTEXT, 0, (LPARAM)("Config Instructions") );	

	SetFocus( hWndDlg );

	// Hide our last window
	ShowWindow( m_pMenuManager->GetLastMenuHandle(), SW_HIDE ); // hide our top menu
	
	// Set Configure Game to false
	SendMessage( GetDlgItem(hWndDlg,IDC_LABEL_ACTION) , BM_SETCHECK, 0, 0);

	return S_OK;
}

void CFrontEnd::ConfigMenu_OnPaint( const HWND hWndDlg )
{
	RECT rc;
	if( GetUpdateRect(hWndDlg, &rc, false ) )
	{
		HDC hDC = GetDC( hWndDlg );
		m_bmpBackground.Draw( hDC, rc.left, rc.top, rc.left, rc.top, rc.right-rc.left, rc.bottom-rc.top );
		ReleaseDC( hWndDlg, hDC );
	}
}

void CFrontEnd::ConfigMenu_OnButtonJoystickConfig( void )
{
	if(FAILED( WinExec("control.exe joy.cpl", SW_NORMAL)) )
	{
		// Explain to the user that we could not open the joystick properties through the control panel.
		MessageBox(m_hWnd, "Could not find Joystick Options in the Control Panel!", "Error Opening Joystick Options", MB_OK);
	}
}

void CFrontEnd::ConfigMenu_OnButtonVideoConfig( void )
{
	if(FAILED( WinExec("control.exe video.cpl", SW_NORMAL)) )
	{
		// Explain to the user that we could not open the joystick properties through the control panel.
		MessageBox(m_hWnd, "Could not find Video Options in the Control Panel!", "Error Opening Joystick Options", MB_OK);
	}
}

INT_PTR CALLBACK ConfigMenu_DialogProc( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam )
{
	switch( iMsg )
	{
		// ************* Windows Messages for Menu Manager ***************
		case WM_MM_OPEN:
			g_FrontEnd.ConfigMenu_OnOpen( hWndDlg );
			return 0;
		
		case WM_MM_CLOSE:
			return 0;
		// ***************************************************************
		
		case WM_PAINT:
			g_FrontEnd.ConfigMenu_OnPaint( hWndDlg );
			break;
		
		case WM_DRAWITEM:
			g_FrontEnd.OnDrawItemButton( hWndDlg, (LPDRAWITEMSTRUCT)lParam );
			break;
		
		case WM_INITDIALOG:
			g_FrontEnd.OnInitDialog( hWndDlg );
			return true;
		
		case WM_NCCALCSIZE:  //CAPTURE THIS MESSAGE AND RETURN NULL
			break;
		
		case WM_COMMAND:
			switch( LOWORD(wParam) )
			{
				case IDC_BUTTON_JOYCONFIG:	 g_FrontEnd.ConfigMenu_OnButtonJoystickConfig( );	break;
				case IDC_BUTTON_VIDEOCONFIG: g_FrontEnd.ConfigMenu_OnButtonVideoConfig( );		break;
				
				case IDC_BUTTON_CANCEL:		 g_FrontEnd.PopCurrentMenu( );						break;
			}
			break;
		
		case WM_CLOSE:
			g_FrontEnd.PopCurrentMenu( );
			return 0;
		
		case WM_DESTROY:
            PostQuitMessage( 0 );
            return 0;
	}
	return false;
}

