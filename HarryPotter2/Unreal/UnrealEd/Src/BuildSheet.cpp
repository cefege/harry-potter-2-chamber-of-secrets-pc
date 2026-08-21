/*=============================================================================
	TBuildSheet : Property sheet for map rebuild options/stats
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:
	- remove the OK and Cancel buttons off the bottom of the window and resize
	  the window accordingly

=============================================================================*/

#include "BuildSheet.h"
#include "res/resource.h"
#define _WIN32_IE 0x0200
#include <commctrl.h>

#include "..\..\Editor\Src\EditorPrivate.h"
EDITOR_API extern class UEditorEngine* GEditor;

BOOL APIENTRY OptionsProc(HWND hDlg, UINT message, UINT wParam, LONG lParam);
BOOL APIENTRY StatsProc(HWND hDlg, UINT message, UINT wParam, LONG lParam);

extern TBuildSheet* GBuildSheet;

HWND GhwndBSPages[eBS_MAX];

TBuildSheet::TBuildSheet()
{
	m_bShow = FALSE;
	m_hwndSheet = NULL;
}

TBuildSheet::~TBuildSheet()
{
}

void TBuildSheet::OpenWindow( HINSTANCE hInst, HWND hWndOwner )
{
	// Options page
	//
	m_pages[eBS_OPTIONS].dwSize = sizeof(PROPSHEETPAGE);
	m_pages[eBS_OPTIONS].dwFlags = PSP_USETITLE;
	m_pages[eBS_OPTIONS].hInstance = hInst;
	m_pages[eBS_OPTIONS].pszTemplate = MAKEINTRESOURCE(IDPP_BUILD_OPTIONS);
	m_pages[eBS_OPTIONS].pszIcon = NULL;
	m_pages[eBS_OPTIONS].pfnDlgProc = OptionsProc;
	m_pages[eBS_OPTIONS].pszTitle = TEXT("Options");
	m_pages[eBS_OPTIONS].lParam = 0;
	GhwndBSPages[eBS_OPTIONS] = NULL;

	// Stats page
	//
	m_pages[eBS_STATS].dwSize = sizeof(PROPSHEETPAGE);
	m_pages[eBS_STATS].dwFlags = PSP_USETITLE;
	m_pages[eBS_STATS].hInstance = hInst;
	m_pages[eBS_STATS].pszTemplate = MAKEINTRESOURCE(IDPP_BUILD_STATS);
	m_pages[eBS_STATS].pszIcon = NULL;
	m_pages[eBS_STATS].pfnDlgProc = StatsProc;
	m_pages[eBS_STATS].pszTitle = TEXT("Stats");
	m_pages[eBS_STATS].lParam = 0;
	GhwndBSPages[eBS_STATS] = NULL;

	// Property sheet
	//
	m_psh.dwSize = sizeof(PROPSHEETHEADER);
	m_psh.dwFlags = PSH_PROPSHEETPAGE | PSH_MODELESS | PSH_NOAPPLYNOW;
	m_psh.hwndParent = hWndOwner;
	m_psh.hInstance = hInst;
	m_psh.pszIcon = NULL;
	m_psh.pszCaption = TEXT("Build");
	m_psh.nPages = eBS_MAX;
	m_psh.ppsp = (LPCPROPSHEETPAGE)&m_pages;

	m_hwndSheet = (HWND)PropertySheet( &m_psh );
	m_bShow = TRUE;

	// Customize the property sheet by deleting the OK button and changing the "Cancel" button to "Hide".
	DestroyWindow( GetDlgItem( m_hwndSheet, IDOK ) );
	SendMessageA( GetDlgItem( m_hwndSheet, IDCANCEL ), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)"&Hide");
}

void TBuildSheet::Show( BOOL bShow )
{
	if( !m_hwndSheet 
			|| m_bShow == bShow ) { return; }

	ShowWindow( m_hwndSheet, bShow ? SW_SHOW : SW_HIDE );

	m_bShow = bShow;
}

void TBuildSheet::Build()
{
	TCHAR Cmd[128], Wk[64];
	UBOOL bVisibleOnly = SendMessageA(::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_GEOMETRY), BM_GETCHECK, 0, 0 ) == BST_CHECKED;

	// GEOMETRY
	if( SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_GEOMETRY), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
	{
		FString Cmd = FString::Printf(TEXT("MAP REBUILD VISIBLEONLY=%d"), bVisibleOnly );
		GEditor->Exec( *Cmd );
	}

	// BSP
	if( SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_BSP), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
	{
		appStrcpy( Cmd, TEXT("BSP REBUILD") );
		if( (SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDRB_LAME), BM_GETCHECK, 0, 0 ) == BST_CHECKED ) )
			appStrcat( Cmd, TEXT(" LAME") );
		if( (SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDRB_GOOD), BM_GETCHECK, 0, 0 ) == BST_CHECKED ) )
			appStrcat( Cmd, TEXT(" GOOD") );
		if( (SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDRB_OPTIMAL), BM_GETCHECK, 0, 0 ) == BST_CHECKED ) )
			appStrcat( Cmd, TEXT(" OPTIMAL") );
		if( (SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_OPT_GEOM), BM_GETCHECK, 0, 0 ) == BST_CHECKED ) )
			appStrcat( Cmd, TEXT(" OPTGEOM") );
		if( (SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_BUILD_VIS_ZONES), BM_GETCHECK, 0, 0 ) == BST_CHECKED ) )
			appStrcat( Cmd, TEXT(" ZONES") );
		appSprintf( Wk, TEXT(" BALANCE=%d"), SendMessageA( GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDSL_BALANCE), TBM_GETPOS, 0, 0 ) );
		appStrcat( Cmd, Wk );
		appSprintf( Wk, TEXT(" PORTALBIAS=%d"), SendMessageA( GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDSL_PORTALBIAS), TBM_GETPOS, 0, 0 ) );
		appStrcat( Cmd, Wk );

		GEditor->Exec( Cmd );
	}

	// LIGHTING
	if( SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_LIGHTING), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
	{
		UBOOL bSelected = SendMessageA(::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_SEL_LIGHTS_ONLY), BM_GETCHECK, 0, 0 ) == BST_CHECKED;
		GEditor->Exec( *(FString::Printf(TEXT("LIGHT APPLY SELECTED=%d VISIBLEONLY=%d"), bSelected, bVisibleOnly) ) );
	}

	// PATHS
	if( SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_PATH_DEFINE), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
		GEditor->Exec( TEXT("PATHS DEFINE") );

	RefreshStats();
}

void TBuildSheet::RefreshStats()
{
	// GEOMETRY
	FStringOutputDevice GetPropResult = FStringOutputDevice();

	GetPropResult.Empty();	GEditor->Get( TEXT("MAP"), TEXT("BRUSHES"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_BRUSHES), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
	GetPropResult.Empty();	GEditor->Get( TEXT("MAP"), TEXT("ZONES"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_ZONES), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );

	// BSP
	int iPolys, iNodes;
	GetPropResult.Empty();	GEditor->Get( TEXT("BSP"), TEXT("POLYS"), GetPropResult );	iPolys = appAtoi( *GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_POLYS), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
	GetPropResult.Empty();	GEditor->Get( TEXT("BSP"), TEXT("NODES"), GetPropResult );	iNodes = appAtoi( *GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_NODES), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
	GetPropResult.Empty();	GEditor->Get( TEXT("BSP"), TEXT("MAXDEPTH"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_MAX_DEPTH), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
	GetPropResult.Empty();	GEditor->Get( TEXT("BSP"), TEXT("AVGDEPTH"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_AVG_DEPTH), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );

	if(!iPolys)
		SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_RATIO), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)"N/A" );
	else
	{
		float fRatio = (iNodes / (float)iPolys);
		SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_RATIO), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *(FString::Printf(TEXT("%1.2f:1"), fRatio)) ) );
	}

	// LIGHTING
	GetPropResult.Empty();	GEditor->Get( TEXT("LIGHT"), TEXT("COUNT"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_STATS], IDSC_LIGHTS), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
}

// --------------------------------------------------------------
//
// OPTIONS
//
// --------------------------------------------------------------

BOOL APIENTRY OptionsProc(HWND hDlg, UINT message, UINT wParam, LONG lParam)
{
	static BOOL bFirstTime = TRUE;

	switch (message)
	{
		case WM_NOTIFY:

			switch(((NMHDR FAR*)lParam)->code) 
			{
				case PSN_SETACTIVE:

					if(bFirstTime) {

						bFirstTime = FALSE;
						GhwndBSPages[eBS_OPTIONS] = hDlg;

						SendMessageA( GetDlgItem( hDlg, IDCK_GEOMETRY), BM_SETCHECK, BST_CHECKED, 0 );
						SendMessageA( GetDlgItem( hDlg, IDCK_BSP), BM_SETCHECK, BST_CHECKED, 0 );
						SendMessageA( GetDlgItem( hDlg, IDCK_LIGHTING), BM_SETCHECK, BST_CHECKED, 0 );
						SendMessageA( GetDlgItem( hDlg, IDCK_PATH_DEFINE), BM_SETCHECK, BST_CHECKED, 0 );

						SendMessageA( GetDlgItem( hDlg, IDRB_OPTIMAL), BM_SETCHECK, BST_CHECKED, 0 );
						SendMessageA( GetDlgItem( hDlg, IDCK_OPT_GEOM), BM_SETCHECK, BST_CHECKED, 0 );
						SendMessageA( GetDlgItem( hDlg, IDCK_BUILD_VIS_ZONES), BM_SETCHECK, BST_CHECKED, 0 );

						SendMessageA( GetDlgItem( hDlg, IDSL_BALANCE), TBM_SETRANGE, 1, MAKELONG(0, 100) );
						SendMessageA( GetDlgItem( hDlg, IDSL_BALANCE), TBM_SETTICFREQ, 10, 0 );
						SendMessageA( GetDlgItem( hDlg, IDSL_BALANCE), TBM_SETPOS, 1, 15 );

						SendMessageA( GetDlgItem( hDlg, IDSL_PORTALBIAS), TBM_SETRANGE, 1, MAKELONG(0, 100) );
						SendMessageA( GetDlgItem( hDlg, IDSL_PORTALBIAS), TBM_SETTICFREQ, 5, 0 );
						SendMessageA( GetDlgItem( hDlg, IDSL_PORTALBIAS), TBM_SETPOS, 1, 70 );
					}
					break;

				case PSN_QUERYCANCEL:
					GBuildSheet->Show( 0 );
					break;
			}
			break;

		case WM_HSCROLL:
			{
				char buffer[10];
				if( (HWND)lParam == GetDlgItem( hDlg, IDSL_BALANCE) )
				{
					::itoa( SendMessageA( GetDlgItem( hDlg, IDSL_BALANCE), TBM_GETPOS, 0, 0 ), buffer, sizeof(buffer) );
					SendMessageA( GetDlgItem( hDlg, IDSC_BALANCE), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)buffer );
				}
				if( (HWND)lParam == GetDlgItem( hDlg, IDSL_PORTALBIAS) )
				{
					::itoa( SendMessageA( GetDlgItem( hDlg, IDSL_PORTALBIAS), TBM_GETPOS, 0, 0 ), buffer, sizeof(buffer) );
					SendMessageA( GetDlgItem( hDlg, IDSC_PORTALBIAS), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)buffer );
				}
			}
			break;

		case WM_COMMAND:

			switch(HIWORD(wParam)) {

				case BN_CLICKED:

					switch(LOWORD(wParam)) {

						case IDCK_GEOMETRY:
							{
								BOOL bChecked = (SendMessageA( GetDlgItem( hDlg, IDCK_GEOMETRY), BM_GETCHECK, 0, 0 ) == BST_CHECKED);

								EnableWindow( GetDlgItem( hDlg, IDCK_ONLY_VISIBLE), bChecked );
							}
							break;

						case IDCK_BSP:
							{
								BOOL bChecked = (SendMessageA( GetDlgItem( hDlg, IDCK_BSP), BM_GETCHECK, 0, 0 ) == BST_CHECKED);

								EnableWindow( GetDlgItem( hDlg, IDSC_OPTIMIZATION), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDRB_LAME), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDRB_GOOD), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDRB_OPTIMAL), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDCK_OPT_GEOM), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDCK_BUILD_VIS_ZONES), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDSL_BALANCE), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDSC_BSP_1), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDSC_BSP_2), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDSC_BALANCE), bChecked );
							}
							break;

						case IDCK_LIGHTING:
							{
								BOOL bChecked = (SendMessageA( GetDlgItem( hDlg, IDCK_LIGHTING), BM_GETCHECK, 0, 0 ) == BST_CHECKED);

								EnableWindow( GetDlgItem( hDlg, IDCK_SEL_LIGHTS_ONLY), bChecked );
								EnableWindow( GetDlgItem( hDlg, IDSC_RESOLUTION), bChecked );
							}
							break;

						case IDPB_BUILD:
							GBuildSheet->Build();
							break;

						case IDPB_BUILD_PATHS:
						{
							if( ::MessageBox( hDlg, TEXT("This command will erase all existing pathnodes and attempt to create a pathnode network on its own.  Are you sure this is what you want to do?\n\nNOTE : This process can take a VERY long time."), TEXT("Build Paths"), MB_YESNO) == IDYES )
							{
								GEditor->Exec( TEXT("PATHS BUILD") );
							}
						}
						break;
					}
			}
			break;
	}

	return (FALSE);
}

// --------------------------------------------------------------
//
// STATS
//
// --------------------------------------------------------------

BOOL APIENTRY StatsProc(HWND hDlg, UINT message, UINT wParam, LONG lParam)
{
	static BOOL bFirstTime = TRUE;

	switch (message)
	{
		case WM_NOTIFY:

			switch(((NMHDR FAR*)lParam)->code) 
			{
				case PSN_SETACTIVE:

					if(bFirstTime) {

						bFirstTime = FALSE;
						GhwndBSPages[eBS_STATS] = hDlg;
						GBuildSheet->RefreshStats();
					}
					break;

				case PSN_QUERYCANCEL:
					GBuildSheet->Show( 0 );
					break;
			}
			break;

		case WM_COMMAND:

			switch(HIWORD(wParam)) {

				case BN_CLICKED:

					switch(LOWORD(wParam)) {

						case IDPB_REFRESH:
							GBuildSheet->RefreshStats();
							break;

						case IDPB_BUILD:
							GBuildSheet->Build();
							break;
					}
			}
			break;
	}

	return (FALSE);
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
