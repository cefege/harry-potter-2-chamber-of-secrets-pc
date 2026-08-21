// -----------------------------------------------------------------------------
//  ______ ______        _                      _  _____                      __  __                                       
// |  ____|  ____|      | |                    | |/ ____|                    |  \/  |                                      
// | |__  | |__         | |      ___   __ _  __| | |  __  __ _ _ __ ___   ___| \  / | ___ _ __  _   _      ___ _ __  _ __  
// |  __| |  __|        | |     / _ \ / _` |/ _` | | |_ |/ _` | '_ ` _ \ / _ \ |\/| |/ _ \ '_ \| | | |    / __| '_ \| '_ \ 
// | |    | |____       | |____| (_) | (_| | (_| | |__| | (_| | | | | | |  __/ |  | |  __/ | | | |_| | _ | (__| |_) | |_) |
// |_|    |______|      |______|\___/ \__,_|\__,_|\_____|\__,_|_| |_| |_|\___|_|  |_|\___|_| |_|\__,_|(_) \___| .__/| .__/ 
//                ______                                                                                      | |   | |    
//               |______|                                                                                     |_|   |_|    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#define WIN32_LEAN_AND_MEAN
#include <windows.h>				//
#include <stdio.h>					// sprintf()
#include <assert.h>

#include ".\common\types.h"			//
#include ".\common\win_utilities.h"	//

#include ".\MenuManager\MenuManager.h"

#include ".\FrontEnd.h"				//
#include ".\resource.h"				//


// *************************************************************************************************************
// *************************************************************************************************************
// -- Load Game Menu Functions

HRESULT CFrontEnd::LoadGameMenu_OnOpen( const HWND hWndDlg )
{
	if( hWndDlg == NULL ) return E_INVALIDARG;

	HWND hWndButton;
	char buffer[32];
	
	// destroy the curent splash screen and load the new one
//	m_bmpBackground.Destroy();
//	if( !m_bmpBackground.LoadBmpFile( BITMAP_FILENAME_LOADGAME_BACKGROUND ) )
//		return E_FAIL;
	
	// Setup the Title of the save slots
	SetWindowText( hWndDlg, ("FrontEnd: Load Game Menu") );
	SendMessage( GetDlgItem(hWndDlg,IDC_LABEL_ACTION),		WM_SETTEXT, 0, (LPARAM)("Load Game Instructions") );
	
	// Setup the Save Slot buttons
	for(int i=1; i<NUM_SAVE_SLOTS; ++i )
	{
		hWndButton = GetDlgItem( hWndDlg, GetResourceIDFromSlotIndex(i));
		
		// If this save slot is empty then disable the button assosiated with it.
		if( m_SaveSlots[i].m_bEmpty )
		{
			EnableWindow( hWndButton, false );
			sprintf(buffer, "%d - Empty", i );
		}
		else
		{
			sprintf(buffer, "%d - Used", i );
		}
		SendMessage( hWndButton, WM_SETTEXT, 0, (LPARAM)(buffer) );
	}

	
	SetFocus( hWndDlg );

	// Hide our last window
	ShowWindow( m_pMenuManager->GetLastMenuHandle(), SW_HIDE ); // hide our top menu
	
	return S_OK;
}

void CFrontEnd::LoadGameMenu_OnButtonSlot( const u32 iSlot )
{
	if( iSlot > NUM_SAVE_SLOTS ) { ErrorMessage("Invalid Slot Number: %d !", iSlot ); return; }

	// Launch our game using the specified slot!
	LaunchLoadGame( iSlot );
}

void CFrontEnd::LoadGameMenu_OnPaint( const HWND hWndDlg )
{
	RECT rc;
	if( GetUpdateRect(hWndDlg, &rc, false ) )
	{
		HDC hDC = GetDC( hWndDlg );
		m_bmpBackground.Draw( hDC, rc.left, rc.top, rc.left, rc.top, rc.right-rc.left, rc.bottom-rc.top );
		ReleaseDC( hWndDlg, hDC );
	}
}

void CFrontEnd::LoadGameMenu_OnDrawItemButtonSlot( HWND hWndDlg, LPDRAWITEMSTRUCT pDIS )
{
	assert( hWndDlg && pDIS );

	char text[256];
	int  iSlot;
	
	iSlot = GetSlotIndexFromResourceID( pDIS->CtlID );

	// *** Draw the button bitmap
	if( m_SaveSlots[ iSlot ].m_bEmpty )
	{
		m_bmpButtonThumbDisabled.Draw( pDIS->hDC, 0, 0 );
	}
	else// --- not empty (if there is a thumnail, draw it)
	{
		if( pDIS->itemState & ODS_SELECTED )
		{
			m_bmpButtonThumbDown.Draw( pDIS->hDC, 0, 0 );
			m_SaveSlots[ iSlot ].m_bmpThumnail.Draw( pDIS->hDC, 2, 2 );
		}
		else
		{
			m_bmpButtonThumbUp.Draw( pDIS->hDC, 0, 0 );
			m_SaveSlots[ iSlot ].m_bmpThumnail.Draw( pDIS->hDC, 1, 1 );
		}

	}
	
	// *** Draw the button text
	SetTextColor( pDIS->hDC, 0xFFFFFF );
	SetBkMode( pDIS->hDC, TRANSPARENT );
	GetDlgItemText( hWndDlg, pDIS->CtlID, text, 256 );
	
	// DT_VCENTER doens't seem to be centering the text, so i'll do it manually.
	int h = pDIS->rcItem.bottom - pDIS->rcItem.top;
	pDIS->rcItem.top    += (h>>1) - 7;
	pDIS->rcItem.bottom += (h>>1) - 7;

	// Offset the text if the button is selected
	if(pDIS->itemState & ODS_SELECTED)
	{
		pDIS->rcItem.left++;
		pDIS->rcItem.right++;
		pDIS->rcItem.top++;
		pDIS->rcItem.bottom++;
	}

	DrawText( pDIS->hDC, text, strlen(text), &pDIS->rcItem, DT_CENTER | DT_VCENTER );
}

INT_PTR CALLBACK LoadGameMenu_DialogProc( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam )
{
	switch( iMsg )
	{
		// ************* Windows Messages for Menu Manager ***************
		case WM_MM_OPEN:
			g_FrontEnd.LoadGameMenu_OnOpen( hWndDlg );
			return 0;
		
		case WM_MM_CLOSE:
			return 0;
		// ***************************************************************
		
		case WM_PAINT:
			g_FrontEnd.LoadGameMenu_OnPaint( hWndDlg );
			break;

		case WM_DRAWITEM:
			if( ((LPDRAWITEMSTRUCT)lParam)->CtlID >= IDC_BUTTON_SLOT0 &&
				((LPDRAWITEMSTRUCT)lParam)->CtlID <= IDC_BUTTON_SLOT7  )
			{
				g_FrontEnd.LoadGameMenu_OnDrawItemButtonSlot( hWndDlg, (LPDRAWITEMSTRUCT)lParam );
			}
			else // Draw a regular button
			{
				g_FrontEnd.OnDrawItemButton( hWndDlg, (LPDRAWITEMSTRUCT)lParam );
			}
			break;

		case WM_INITDIALOG:
			g_FrontEnd.OnInitDialog( hWndDlg );
			return true;
		
		case WM_NCCALCSIZE:  //CAPTURE THIS MESSAGE AND RETURN NULL
			break;
		
		case WM_COMMAND:
			switch( LOWORD(wParam) )
			{
				case IDC_BUTTON_SLOT1:	g_FrontEnd.LoadGameMenu_OnButtonSlot( 1 );	break;
				case IDC_BUTTON_SLOT2:	g_FrontEnd.LoadGameMenu_OnButtonSlot( 2 );	break;
				case IDC_BUTTON_SLOT3:	g_FrontEnd.LoadGameMenu_OnButtonSlot( 3 );	break;
				case IDC_BUTTON_SLOT4:	g_FrontEnd.LoadGameMenu_OnButtonSlot( 4 );	break;
				case IDC_BUTTON_SLOT5:	g_FrontEnd.LoadGameMenu_OnButtonSlot( 5 );	break;
				case IDC_BUTTON_SLOT6:	g_FrontEnd.LoadGameMenu_OnButtonSlot( 6 );	break;
				
				case IDC_BUTTON_CANCEL:	g_FrontEnd.PopCurrentMenu();	break;
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



