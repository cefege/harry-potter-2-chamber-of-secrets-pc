// -----------------------------------------------------------------------------
//   _____       _           _     __  __                                       
//  / ____|     | |         | |   |  \/  |                                      
// | (___  _ __ | | __ _ ___| |__ | \  / | ___ _ __  _   _      ___ _ __  _ __  
//  \___ \| '_ \| |/ _` / __| '_ \| |\/| |/ _ \ '_ \| | | |    / __| '_ \| '_ \ 
//  ____) | |_) | | (_| \__ \ | | | |  | |  __/ | | | |_| | _ | (__| |_) | |_) |
// |_____/| .__/|_|\__,_|___/_| |_|_|  |_|\___|_| |_|\__,_|(_) \___| .__/| .__/ 
//        | |                                                      | |   | |    
//        |_|                                                      |_|   |_|    
//
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
// -- Load Game Menu Functions

HRESULT CFrontEnd::Splash_OnOpen( const HWND hWndDlg )
{
	if( hWndDlg == NULL ) return E_INVALIDARG;
	
	// Setup the Title of the save slots
	SetWindowText( hWndDlg, ("FrontEnd: Splash Screen") );
	
	// Create the first splash screen
	m_bmpBackground.Destroy();
	if( !m_bmpBackground.LoadBmpFile( GetBitmapPath( m_strSplashQueue[m_iSplashIndex] ) ) )
	{
		PopCurrentMenu(); // close this menu
		return E_FAIL;
	}
	
	Util_SetWindow( hWndDlg, 0.5f, 0.5f, m_bmpBackground.GetWidth(), m_bmpBackground.GetHeight() );
	
	// Set a timer for when we will end the first splash screen
	SetTimer( hWndDlg, NULL, SPLASH_WAIT_TIME, NULL );
	
	return S_OK;
}

void CFrontEnd::Splash_OnPaint( const HWND hWndDlg )
{
	HDC hDC = GetDC(hWndDlg);
	m_bmpBackground.Draw( hDC, 0, 0 );
	ReleaseDC( hWndDlg, hDC );
}

void CFrontEnd::Splash_OnTimer( const HWND hWndDlg )
{
	// Inc our splash queue
	m_iSplashIndex++;
	if( m_iSplashIndex < SPLASH_MAX_SCREENS && 
		m_strSplashQueue[m_iSplashIndex][0] )
	{
		// destroy the curent splash screen and load the new one
		m_bmpBackground.Destroy();
		if( !m_bmpBackground.LoadBmpFile( GetBitmapPath( m_strSplashQueue[m_iSplashIndex] )) )
		{
			KillTimer( hWndDlg, NULL );	// kill our splash timer
			PopCurrentMenu();			// pop our current menu
			return;
		}
		
		InvalidateRect(hWndDlg, NULL, false);				// ReDraw screen with the new bitmap
		SetTimer( hWndDlg, NULL, SPLASH_WAIT_TIME, NULL );	// reset timer
	}
	else
	{
		KillTimer( hWndDlg, NULL );			// kill our splash timer
		PopCurrentMenu();					// pop our current menu
	}
}

void CFrontEnd::Splash_OnClose( void )
{
	m_bmpBackground.Destroy();
}

INT_PTR CALLBACK Splash_DialogProc( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam )
{
	switch( iMsg )
	{
		// ************* Windows Messages for Menu Manager ***************
		case WM_MM_OPEN:
			g_FrontEnd.Splash_OnOpen( hWndDlg );
			return 1;
		
		case WM_MM_CLOSE:
			return 1;
		// ***************************************************************
		
		case WM_INITDIALOG:
			g_FrontEnd.OnInitDialog( hWndDlg );
			return true;

		case WM_CLOSE:
			g_FrontEnd.Splash_OnClose();
			return 0;
		
		case WM_TIMER:
			g_FrontEnd.Splash_OnTimer( hWndDlg );
			break;

		case WM_PAINT:
			g_FrontEnd.Splash_OnPaint( hWndDlg );
			break;
				
		case WM_DESTROY:
			PostQuitMessage( 0 );
			return 0;
	}
	return false;
}
