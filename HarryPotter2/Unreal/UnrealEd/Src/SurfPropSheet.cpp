/*=============================================================================
	TSurfPropSheet : Property sheet for manipulating poly properties
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include "SurfPropSheet.h"
#include "res/resource.h"
#define _WIN32_IE 0x0200
#include <commctrl.h>

#include "..\..\Editor\Src\EditorPrivate.h"
EDITOR_API extern class UEditorEngine* GEditor;

BOOL APIENTRY Flags1Proc(HWND hDlg, UINT message, UINT wParam, LONG lParam);
BOOL APIENTRY AlignmentProc(HWND hDlg, UINT message, UINT wParam, LONG lParam);
BOOL APIENTRY SPStatsProc(HWND hDlg, UINT message, UINT wParam, LONG lParam);
void GetDataFromSurfs1( HWND hDlg, ULevel* Level );

extern TSurfPropSheet* GSurfPropSheet;

HWND GhwndSPPages[eSPS_MAX];

void UpdateChildControls()
{
	FStringOutputDevice GetPropResult = FStringOutputDevice();
	GetPropResult.Empty();
	if( GEditor )
		GEditor->Get( TEXT("POLYS"), TEXT("NUMSELECTED"), GetPropResult );
	int NumSelected = appAtoi(*GetPropResult);

	// Disable all controls if no surfaces are selected.
	HWND hwndChild;
	for( int x = 0 ; x < eSPS_MAX ; x++ )
	{
		hwndChild = GetWindow( GhwndSPPages[x], GW_CHILD );
		while( hwndChild )
		{
			EnableWindow( hwndChild, NumSelected );
			hwndChild = GetWindow( hwndChild, GW_HWNDNEXT );
		}
	}

	// Change caption to show how many surfaces are selected.
	GetPropResult.Empty();
	if( GEditor )
		GEditor->Get( TEXT("POLYS"), TEXT("TEXTURENAME"), GetPropResult );
	FString Caption;
	if( NumSelected == 1 )
		Caption = FString::Printf(TEXT("%d Surface%s%s"), NumSelected, GetPropResult.Len() ? TEXT(" : ") : TEXT(""), *GetPropResult );
	else
		Caption = FString::Printf(TEXT("%d Surfaces%s%s"), NumSelected, GetPropResult.Len() ? TEXT(" : ") : TEXT(""), *GetPropResult );
	SendMessageA( GSurfPropSheet->m_hwndSheet, WM_SETTEXT, 0, (LPARAM)appToAnsi( *Caption ) );
}

TSurfPropSheet::TSurfPropSheet()
{
	m_bShow = FALSE;
	m_hwndSheet = NULL;
}

TSurfPropSheet::~TSurfPropSheet()
{
}

void TSurfPropSheet::OpenWindow( HINSTANCE hInst, HWND hWndOwner )
{
	// Flags1 page
	//
	m_pages[eSPS_FLAGS1].dwSize = sizeof(PROPSHEETPAGE);
	m_pages[eSPS_FLAGS1].dwFlags = PSP_USETITLE;
	m_pages[eSPS_FLAGS1].hInstance = hInst;
	m_pages[eSPS_FLAGS1].pszTemplate = MAKEINTRESOURCE(IDPP_SP_FLAGS1);
	m_pages[eSPS_FLAGS1].pszIcon = NULL;
	m_pages[eSPS_FLAGS1].pfnDlgProc = Flags1Proc;
	m_pages[eSPS_FLAGS1].pszTitle = TEXT("Flags");
	m_pages[eSPS_FLAGS1].lParam = 0;
	GhwndSPPages[eSPS_FLAGS1] = NULL;

	// Alignment page
	//
	m_pages[eSPS_ALIGNMENT].dwSize = sizeof(PROPSHEETPAGE);
	m_pages[eSPS_ALIGNMENT].dwFlags = PSP_USETITLE;
	m_pages[eSPS_ALIGNMENT].hInstance = hInst;
	m_pages[eSPS_ALIGNMENT].pszTemplate = MAKEINTRESOURCE(IDPP_SP_ALIGNMENT);
	m_pages[eSPS_ALIGNMENT].pszIcon = NULL;
	m_pages[eSPS_ALIGNMENT].pfnDlgProc = AlignmentProc;
	m_pages[eSPS_ALIGNMENT].pszTitle = TEXT("Alignment");
	m_pages[eSPS_ALIGNMENT].lParam = 0;
	GhwndSPPages[eSPS_ALIGNMENT] = NULL;

	// Stats page
	//
	m_pages[eSPS_STATS].dwSize = sizeof(PROPSHEETPAGE);
	m_pages[eSPS_STATS].dwFlags = PSP_USETITLE;
	m_pages[eSPS_STATS].hInstance = hInst;
	m_pages[eSPS_STATS].pszTemplate = MAKEINTRESOURCE(IDPP_SP_STATS);
	m_pages[eSPS_STATS].pszIcon = NULL;
	m_pages[eSPS_STATS].pfnDlgProc = SPStatsProc;
	m_pages[eSPS_STATS].pszTitle = TEXT("Stats");
	m_pages[eSPS_STATS].lParam = 0;
	GhwndSPPages[eSPS_STATS] = NULL;

	// Property sheet
	//
	m_psh.dwSize = sizeof(PROPSHEETHEADER);
	m_psh.dwFlags = PSH_PROPSHEETPAGE | PSH_MODELESS | PSH_NOAPPLYNOW;
	m_psh.hwndParent = hWndOwner;
	m_psh.hInstance = hInst;
	m_psh.pszIcon = NULL;
	m_psh.pszCaption = TEXT("Surface Properties");
	m_psh.nPages = eSPS_MAX;
	m_psh.ppsp = (LPCPROPSHEETPAGE)&m_pages;

	m_hwndSheet = (HWND)PropertySheet( &m_psh );
	m_bShow = TRUE;

	// Customize the property sheet by deleting the OK button and changing the "Cancel" button to "Hide".
	DestroyWindow( GetDlgItem( m_hwndSheet, IDOK ) );
	SendMessageA( GetDlgItem( m_hwndSheet, IDCANCEL ), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)"&Hide");
}

void TSurfPropSheet::Show( BOOL bShow )
{
	if( !m_hwndSheet 
			|| m_bShow == bShow ) { return; }

	ShowWindow( m_hwndSheet, bShow ? SW_SHOW : SW_HIDE );

	if( bShow )
	{
		::GetDataFromSurfs1( GhwndSPPages[eSPS_FLAGS1], GEditor->Level );
		RefreshStats();
	}

	m_bShow = bShow;
}

void TSurfPropSheet::GetDataFromSurfs1()
{
	::GetDataFromSurfs1( GhwndSPPages[eSPS_FLAGS1], GEditor->Level );
}

void TSurfPropSheet::RefreshStats()
{
	FStringOutputDevice GetPropResult = FStringOutputDevice();

	GetPropResult.Empty();	GEditor->Get( TEXT("POLYS"), TEXT("STATICLIGHTS"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndSPPages[eSPS_STATS], IDSC_STATIC_LIGHTS), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
	GetPropResult.Empty();	GEditor->Get( TEXT("POLYS"), TEXT("MESHELS"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndSPPages[eSPS_STATS], IDSC_MESHELS), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );
	GetPropResult.Empty();	GEditor->Get( TEXT("POLYS"), TEXT("MESHSIZE"), GetPropResult );
	SendMessageA( ::GetDlgItem( GhwndSPPages[eSPS_STATS], IDSC_MESH_SIZE), WM_SETTEXT, 0, (LPARAM)(LPCTSTR)TCHAR_TO_ANSI( *GetPropResult ) );

	UpdateChildControls();
}

// --------------------------------------------------------------
//
// FLAGS1
//
// --------------------------------------------------------------

#define dNUM_FLAGS1	21
struct {
	int Flag;		// Unreal's bit flag
	int ID;			// Windows control ID
	int Count;		// Temp var
} GPolyflags1[] = {
	PF_Invisible,			IDCK_INVISIBLE,			0,
	PF_Masked,				IDCK_MASKED,			0,
	PF_Translucent,			IDCK_TRANSLUCENT,		0,
	PF_ForceViewZone,		IDCK_FORCEVIEWZONE,		0,
	PF_Modulated,			IDCK_MODULATED,			0,
	PF_FakeBackdrop,		IDCK_FAKEBACKDROP,		0,
	PF_TwoSided,			IDCK_2SIDED,			0,
	PF_AutoUPan,			IDCK_UPAN,				0,
	PF_AutoVPan,			IDCK_VPAN,				0,
	PF_NoSmooth,			IDCK_NOSMOOTH,			0,
	PF_SpecialPoly,			IDCK_SPECIALPOLY,		0,
	PF_SmallWavy,			IDCK_SMALLWAVY,			0,
	PF_LowShadowDetail,		IDCK_LOWSHADOWDETAIL,	0,
	PF_DirtyShadows,		IDCK_DIRTYSHADOWS,		0,
	PF_BrightCorners,		IDCK_BRIGHTCORNERS,		0,
	PF_SpecialLit,			IDCK_SPECIALLIT,		0,
	PF_NoBoundRejection,	IDCK_NOBOUNDREJECTION,	0,
	PF_Unlit,				IDCK_UNLIT,				0,
	PF_HighShadowDetail,	IDCK_HISHADOWDETAIL,	0,
	PF_Portal,				IDCK_PORTAL,			0,
	PF_Mirrored,			IDCK_MIRROR,			0
};

void GetDataFromSurfs1( HWND hDlg, ULevel* Level )
{
	if( !hDlg ) return;

	int TotalSurfs = 0, x;

	// Init counts.
	//
	for( x = 0 ; x < dNUM_FLAGS1 ; x++ )
	{
		GPolyflags1[x].Count = 0;
	}

	// Check to see which flags are used on all selected surfaces.
	//
	for( INT i=0; i<Level->Model->Surfs.Num(); i++ )
	{
		FBspSurf *Poly = &Level->Model->Surfs(i);
		if( Poly->PolyFlags & PF_Selected )
		{
			for( x = 0 ; x < dNUM_FLAGS1 ; x++ )
			{
				if( Poly->PolyFlags & GPolyflags1[x].Flag )
				{
					GPolyflags1[x].Count++;
				}
			}

			TotalSurfs++;
		}
	}

	// Update checkboxes on dialog to match selections.
	//
	for( x = 0 ; x < dNUM_FLAGS1 ; x++ )
	{
		SendMessageA( GetDlgItem( hDlg, GPolyflags1[x].ID ), BM_SETCHECK, BST_UNCHECKED, 0 );

		if( TotalSurfs > 0
				&& GPolyflags1[x].Count > 0 )
		{
			if( GPolyflags1[x].Count == TotalSurfs )
				SendMessageA( GetDlgItem( hDlg, GPolyflags1[x].ID ), BM_SETCHECK, BST_CHECKED, 0 );
			else
				SendMessageA( GetDlgItem( hDlg, GPolyflags1[x].ID ), BM_SETCHECK, BST_INDETERMINATE, 0 );
		}
	}
}

void SendDataToSurfs1( HWND hDlg, ULevel* Level )
{    
	int OnFlags, OffFlags;

	OnFlags = OffFlags = 0;

	for( int x = 0 ; x < dNUM_FLAGS1 ; x++ )
	{
		if( SendMessageA( GetDlgItem( hDlg, GPolyflags1[x].ID ), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
			OnFlags += GPolyflags1[x].Flag;
		if( SendMessageA( GetDlgItem( hDlg, GPolyflags1[x].ID ), BM_GETCHECK, 0, 0 ) == BST_UNCHECKED )
			OffFlags += GPolyflags1[x].Flag;
	}

    GEditor->Exec( *(FString::Printf(TEXT("POLY SET SETFLAGS=%d CLEARFLAGS=%d"), OnFlags, OffFlags)) );
}

BOOL APIENTRY Flags1Proc(HWND hDlg, UINT message, UINT wParam, LONG lParam)
{
	static BOOL bFirstTime = TRUE;

	switch (message)
	{
		case WM_NOTIFY:

			switch(((NMHDR FAR*)lParam)->code) 
			{
				case PSN_SETACTIVE:

					if(bFirstTime)
					{
						bFirstTime = FALSE;
						GhwndSPPages[eSPS_FLAGS1] = hDlg;
						UpdateChildControls();
					}
					break;

				case PSN_QUERYCANCEL:
					GSurfPropSheet->Show( 0 );
					break;
			}
			break;

		case WM_COMMAND:

			switch(HIWORD(wParam)) {

				case BN_CLICKED:

					switch(LOWORD(wParam)) {

						// MISC
						//
						case IDCANCEL:
							GSurfPropSheet->Show( FALSE );
							SendDataToSurfs1( hDlg, GEditor->Level );
							break;

						case IDCK_INVISIBLE:
						case IDCK_MASKED:
						case IDCK_TRANSLUCENT:
						case IDCK_FORCEVIEWZONE:
						case IDCK_MODULATED:
						case IDCK_FAKEBACKDROP:
						case IDCK_2SIDED:
						case IDCK_UPAN:
						case IDCK_VPAN:
						case IDCK_NOSMOOTH:
						case IDCK_SPECIALPOLY:
						case IDCK_SMALLWAVY:
						case IDCK_LOWSHADOWDETAIL:
						case IDCK_DIRTYSHADOWS:
						case IDCK_BRIGHTCORNERS:
						case IDCK_SPECIALLIT:
						case IDCK_NOBOUNDREJECTION:
						case IDCK_UNLIT:
						case IDCK_HISHADOWDETAIL:
						case IDCK_PORTAL:
						case IDCK_MIRROR:

							// Don't allow the user to select the BST_INDETERMINATE mode for the check boxes.  The
							// editor uses that to show conflicts.
							//
							if( SendMessageA( GetDlgItem( hDlg, LOWORD(wParam) ), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
								SendMessageA( GetDlgItem( hDlg, LOWORD(wParam) ), BM_SETCHECK, BST_CHECKED, 0 );
							else
								SendMessageA( GetDlgItem( hDlg, LOWORD(wParam) ), BM_SETCHECK, BST_UNCHECKED, 0 );

							SendDataToSurfs1( hDlg, GEditor->Level );
							break;

					}
			}
			break;
	}

	return (FALSE);
}

// --------------------------------------------------------------
//
// ALIGNMENT
//
// --------------------------------------------------------------

BOOL APIENTRY AlignmentProc(HWND hDlg, UINT message, UINT wParam, LONG lParam)
{
	TCHAR l_chCmd[256];
	static BOOL bFirstTime = TRUE;

	switch (message)
	{
		case WM_NOTIFY:

			switch(((NMHDR FAR*)lParam)->code) 
			{
				case PSN_SETACTIVE:

					if(bFirstTime) {

						bFirstTime = FALSE;

						// Load up initial values.
						//
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"0.0625" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"0.125" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"0.25" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"0.5" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"1.0" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"2.0" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"4.0" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"8.0" );
						SendMessageA(GetDlgItem( hDlg, IDCB_SIMPLE_SCALE), CB_ADDSTRING, 0, (LPARAM)"16.0" );

						GhwndSPPages[eSPS_ALIGNMENT] = hDlg;
						UpdateChildControls();
					}
					break;

				case PSN_QUERYCANCEL:
					GSurfPropSheet->Show( 0 );
					break;
			}
			break;

		case WM_COMMAND:

			switch(HIWORD(wParam)) {

				case BN_CLICKED:

					float l_fUU, l_fVV, l_fUV, l_fVU;
					float l_fMod = GetAsyncKeyState(VK_SHIFT) & 0x8000 ? -1 : 1;

					switch(LOWORD(wParam)) {

						// MISC
						//
						case IDCANCEL:
							GSurfPropSheet->Show( FALSE );
							break;

						// SCALING
						//
						case IDPB_SCALE_APPLY:
							{
								char l_chU[20], l_chV[20];
								float l_fU, l_fV;

								GetWindowTextA( GetDlgItem( hDlg, IDEC_SCALE_U ), l_chU, sizeof(l_chV));
								GetWindowTextA( GetDlgItem( hDlg, IDEC_SCALE_V ), l_chV, sizeof(l_chV));

								l_fU = ::atof( l_chU );
								l_fV = ::atof( l_chV );

								if( !l_fU || !l_fV ) { break; }

								l_fU = 1.0f / l_fU;
								l_fV = 1.0f / l_fV;

								if( SendMessageA( GetDlgItem( hDlg, IDCK_RELATIVE ), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
									appSprintf( l_chCmd, TEXT("POLY TEXSCALE RELATIVE UU=%f VV=%f"), l_fU, l_fV );
								else
									appSprintf( l_chCmd, TEXT("POLY TEXSCALE UU=%f VV=%f"), l_fU, l_fV );

								GEditor->Exec( l_chCmd );
							}
							break;

						case IDPB_SCALE_APPLY2:
							{
								char l_chU[20], l_chV[20];
								float l_fU, l_fV;

								GetWindowTextA( GetDlgItem( hDlg, IDCB_SIMPLE_SCALE ), l_chU, sizeof(l_chV));
								GetWindowTextA( GetDlgItem( hDlg, IDCB_SIMPLE_SCALE ), l_chV, sizeof(l_chV));

								l_fU = ::atof( l_chU );
								l_fV = ::atof( l_chV );

								if( !l_fU || !l_fV ) { break; }

								l_fU = 1.0f / l_fU;
								l_fV = 1.0f / l_fV;

								if( SendMessageA( GetDlgItem( hDlg, IDCK_RELATIVE ), BM_GETCHECK, 0, 0 ) == BST_CHECKED )
									appSprintf( l_chCmd, TEXT("POLY TEXSCALE RELATIVE UU=%f VV=%f"), l_fU, l_fV );
								else
									appSprintf( l_chCmd, TEXT("POLY TEXSCALE UU=%f VV=%f"), l_fU, l_fV );

								GEditor->Exec( l_chCmd );
							}
							break;

						// ROTATIONS
						//
						case IDPB_ROT_45:
							l_fUU = 1.0f / appSqrt(2);
							l_fVV = 1.0f / appSqrt(2);
							l_fUV = (1.0f / appSqrt(2)) * l_fMod;
							l_fVU = -(1.0f / appSqrt(2)) * l_fMod;

							appSprintf( l_chCmd, TEXT("POLY TEXMULT UU=%f VV=%f UV=%f VU=%f"),
								l_fUU, l_fVV, l_fUV, l_fVU );
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_ROT_90:
							l_fUU = 0;
							l_fVV = 0;
							l_fUV = 1 * l_fMod;
							l_fVU = -1 * l_fMod;

							appSprintf( l_chCmd, TEXT("POLY TEXMULT UU=%f VV=%f UV=%f VU=%f"),
								l_fUU, l_fVV, l_fUV, l_fVU );
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_ROT_FLIP_U:
							GEditor->Exec( TEXT("POLY TEXMULT UU=-1 VV=1") );
							break;

						case IDPB_ROT_FLIP_V:
							GEditor->Exec( TEXT("POLY TEXMULT UU=1 VV=-1") );
							break;

						// PANNING
						//
						case IDPB_PAN_U_1:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN U=%f"), 1 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_U_4:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN U=%f"), 4 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_U_16:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN U=%f"), 16 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_U_64:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN U=%f"), 64 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_V_1:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN V=%f"), 1 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_V_4:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN V=%f"), 4 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_V_16:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN V=%f"), 16 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						case IDPB_PAN_V_64:
							appSprintf( l_chCmd, TEXT("POLY TEXPAN V=%f"), 64 * l_fMod);
							GEditor->Exec( l_chCmd );
							break;

						// ALIGNMENT
						//
						case IDPB_ALIGN_FLOOR:
							GEditor->Exec( TEXT("POLY TEXALIGN FLOOR") );
							break;

						case IDPB_ALIGN_WALLDIR:
							GEditor->Exec( TEXT("POLY TEXALIGN WALLDIR") );
							break;

						case IDPB_ALIGN_WALLPAN:
							GEditor->Exec( TEXT("POLY TEXALIGN WALLPAN") );
							break;

						case IDPB_ALIGN_UNALIGN:
							GEditor->Exec( TEXT("POLY TEXALIGN DEFAULT") );
							break;

					}
					break;
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

BOOL APIENTRY SPStatsProc(HWND hDlg, UINT message, UINT wParam, LONG lParam)
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
						GhwndSPPages[eSPS_STATS] = hDlg;
						UpdateChildControls();
					}
					break;

				case PSN_QUERYCANCEL:
					GSurfPropSheet->Show( 0 );
					break;
			}
			break;
	}

	return (FALSE);
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
