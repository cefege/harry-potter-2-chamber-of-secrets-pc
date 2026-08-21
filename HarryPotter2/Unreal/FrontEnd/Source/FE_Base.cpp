// -----------------------------------------------------------------------------
//  ______ ______        ____                                      
// |  ____|  ____|      |  _ \                                     
// | |__  | |__         | |_) | __ _ ___  ___      ___ _ __  _ __  
// |  __| |  __|        |  _ < / _` / __|/ _ \    / __| '_ \| '_ \ 
// | |    | |____       | |_) | (_| \__ \  __/ _ | (__| |_) | |_) |
// |_|    |______|      |____/ \__,_|___/\___|(_) \___| .__/| .__/ 
//                ______                              | |   | |    
//               |______|                             |_|   |_|    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#define WIN32_LEAN_AND_MEAN
#include <windows.h>				// 
#include <shellapi.h>				// ShellExecute()
#include <stdio.h>					// sprintf()

#include <assert.h>

#include ".\common\types.h"			
#include ".\common\win_utilities.h"	

#include ".\MenuManager\MenuManager.h"

#include ".\FrontEnd.h"				
#include ".\resource.h"				 


// ******************
// *** PROTOTYPES ***
// ******************
INT_PTR CALLBACK	Splash_DialogProc		( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );
INT_PTR CALLBACK	MainMenu_DialogProc		( HWND hWndDlg, UINT iMsg, WPARAM wParam, LPARAM lParam );

// *************************************************************************************************************
// *** Private Functions
void CFrontEnd::Clear()
{
	m_hWnd				= NULL;
	m_strCommandLine[0] = NULL;
	
	m_iSplashIndex		= 0;
	
	int i;
	for(i=0; i < SPLASH_MAX_SCREENS; ++i)
        m_strSplashQueue[i][0] = NULL;

	for(i=0; i < NUM_SAVE_SLOTS; ++i)
	{
		m_SaveSlots[i].m_bEmpty  = true;
		m_SaveSlots[i].m_Path[0] = NULL;
	}
}

char* CFrontEnd::GetBitmapPath( char* strFilename )
{
	static char BitmapPath[1024] = { NULL };
	static char BitmapFile[1024] = { NULL };
	
	if( !BitmapPath[0] )
	{
		strcpy( BitmapPath, Util_GetBaseDir( m_hInstance ) );
		strcat( BitmapPath, BITMAP_PATH );
	}
	
	sprintf( BitmapFile, "%s\\%s", BitmapPath, strFilename );
	return BitmapFile;
}

// *************************************************************************************************************
// *** Public Functions


HRESULT	CFrontEnd::Init( const HINSTANCE hInstance, const char* strCommandLine )
{
	if( hInstance == NULL )
		return E_INVALIDARG;
	
	
	// --- Save our init paramaters
	m_hInstance = hInstance;
	strcpy(m_strCommandLine, strCommandLine );
	
	
	// --- Setup our SavePath so we can look through it
	char SavePath[1024];
	strcpy(SavePath, Util_GetPersonalDir() );	// start with the personal dir
	if( SavePath[0] && Util_IsOSVer2kOrXP() )
	{
		strcat(SavePath, DEFAULT_GAME_DIR );	// append the user dir
		Util_CreateDirectoy( SavePath );		// make sure we have the game dir
		strcat(SavePath, DEFAULT_SAVE_DIR );	// append the save dir
		Util_CreateDirectoy( SavePath );		// make sure we have the save dir
	}
	else // if we can't use the personal dir then use "Unreal\Save\"
	{
		strcpy(SavePath, Util_GetBaseDir( hInstance ) );	// start with the base dir
		if( !SavePath[0] ) MessageBox( NULL, "Can't find the personal folder OR the base folder!", "ERROR", MB_OK );
		
		strcat(SavePath, "..\\Save" );			// append the save dir
		Util_CreateDirectoy( SavePath );		// make sure we have the save dir		
	}

	// --- Setup our splash bitmap filename queue (for loading later)
	strcpy( m_strSplashQueue[0], BITMAP_FILENAME_SPLASH_0 );
	strcpy( m_strSplashQueue[1], BITMAP_FILENAME_SPLASH_1 );
	
	// --- Load our button bitmaps
	m_bmpButtonUp.LoadBmpFile( GetBitmapPath( BITMAP_FILENAME_BUTTON_UP ) );	
	m_bmpButtonDown.LoadBmpFile( GetBitmapPath( BITMAP_FILENAME_BUTTON_DOWN ) );
	m_bmpButtonDisabled.LoadBmpFile( GetBitmapPath( BITMAP_FILENAME_BUTTON_DISABLED ) );
	
	m_bmpButtonThumbUp.LoadBmpFile( GetBitmapPath( BITMAP_FILENAME_BUTTON_THUMB_UP ) );	
	m_bmpButtonThumbDown.LoadBmpFile( GetBitmapPath( BITMAP_FILENAME_BUTTON_THUMB_DOWN ) );
	m_bmpButtonThumbDisabled.LoadBmpFile( GetBitmapPath( BITMAP_FILENAME_BUTTON_THUMB_DISABLED ) );
	
	// --- Init our SaveSlots
	char strThumbnailFile[1024];
	for(int i=1; i < NUM_SAVE_SLOTS; ++i )
	{
		// Add our slot dir to the SavePath then make sure its there.
		sprintf( m_SaveSlots[i].m_Path, "%s\\Slot%d\\", SavePath, i );
		Util_CreateDirectoy( m_SaveSlots[i].m_Path );
		
		// Create our strThumbnailFile file path
		sprintf(strThumbnailFile, "%s\\%s", m_SaveSlots[i].m_Path, DEFAULT_THUMBNAIL_FILENAME );
		
		// See if this saveSlot has a save game file
		strcat(m_SaveSlots[i].m_Path, DEFAULT_SAVE_FILENAME );
		m_SaveSlots[i].m_bEmpty = ( Util_FileSize( m_SaveSlots[i].m_Path ) > 0 ) ? false : true;
		
		// If this slot is empty clean out its files! ( just to make sure, as we can't have any pa files lingering )
		if( m_SaveSlots[i].m_bEmpty )
			EmptySaveSlotFiles( i );
		
		// If this slot is empty clean out its files! ( just to make sure, as we can't have any pa files lingering )
		if( m_SaveSlots[i].m_bEmpty )
		{
			EmptySaveSlotFiles( i );
			m_SaveSlots[i].m_bmpThumnail.Destroy();
		}
		else
		{
			// Blend our thumbnails
/*			CBitmap thumb;
			thumb.LoadBmpFile( strThumbnailFile );
			// This slot is not empty so lets try to load the strThumbnailFile file
			m_SaveSlots[i].m_bmpThumnail.CreateBmp( thumb.GetWidth(), thumb.GetHeight(), 3 );
			m_SaveSlots[i].m_bmpThumnail.BlendBmpsTranslucent( thumb, m_bmpButtonThumbUp, 0.25f, 0, 0 );
			thumb.Destroy();
*/			
			m_SaveSlots[i].m_bmpThumnail.LoadBmpFile( strThumbnailFile );
		}
	}
	
	// *** Create our Menu Manager ***
	m_pMenuManager = CreateMenuManager( hInstance );
	if( m_pMenuManager == NULL )
		return E_FAIL;
	
	// --- Push our Main Menu onto the stack
	IMenu* pMenuMain = m_pMenuManager->PushMenu( "MainMenu", MAKEINTRESOURCE(IDD_DIALOG_MAINMENU), MainMenu_DialogProc );
	if( pMenuMain == NULL )	return E_FAIL;
	
	// --- Push our splash Menu onto the stack
	IMenu* pMenuSplash = m_pMenuManager->PushMenu( "SplashScreen", MAKEINTRESOURCE(IDD_DIALOG_SPLASH), Splash_DialogProc );
	if( pMenuSplash == NULL ) return E_FAIL;
	
	return S_OK;
}

void CFrontEnd::Destroy( void )
{
	// *** Clean up ***

	// CBitmaps
	for(int i=0; i < NUM_SAVE_SLOTS; ++i)
		m_SaveSlots[i].m_bmpThumnail.Destroy();
	
	
	m_bmpButtonUp.Destroy();	
	m_bmpButtonDown.Destroy();
	m_bmpButtonDisabled.Destroy();
	
	m_bmpButtonThumbUp.Destroy( );	
	m_bmpButtonThumbDown.Destroy( );
	m_bmpButtonThumbDisabled.Destroy( );
	
	m_bmpBackground.Destroy();
	
	// Menu Manager interface
	SAFE_RELEASE( m_pMenuManager );
}


void CFrontEnd::PopCurrentMenu( void )
{
	assert( m_pMenuManager );
	
	// destroy bitmap
	m_bmpBackground.Destroy();
	
	// Pop a menu off of the stack
	m_pMenuManager->PopMenu();
	
	// If there are no more menus to display then post a Quit Message.
	if( m_pMenuManager->GetNumMenus() == 0 )
		PostQuitMessage( 0 );
}

void CFrontEnd::Update( void )
{
	assert( m_pMenuManager );
	
	// Update our menu manager
	m_pMenuManager->Update();
}

u32 CFrontEnd::GetResourceIDFromSlotIndex( const u32 iSlot )
{
	if( iSlot > NUM_SAVE_SLOTS ) { ErrorMessage("Invalid Slot Number: %d !", iSlot ); return 0; }

	switch( iSlot )
	{
		case 1:	return IDC_BUTTON_SLOT1;
		case 2:	return IDC_BUTTON_SLOT2;
		case 3:	return IDC_BUTTON_SLOT3;
		case 4:	return IDC_BUTTON_SLOT4;
		case 5:	return IDC_BUTTON_SLOT5;
		case 6:	return IDC_BUTTON_SLOT6;
	}
	return 0;
}
u32 CFrontEnd::GetSlotIndexFromResourceID( const u32 iResourceID )
{
	switch( iResourceID )
	{
		case IDC_BUTTON_SLOT1:	return 1;
		case IDC_BUTTON_SLOT2:	return 2;
		case IDC_BUTTON_SLOT3:	return 3;
		case IDC_BUTTON_SLOT4:	return 4;
		case IDC_BUTTON_SLOT5:	return 5;
		case IDC_BUTTON_SLOT6:	return 6;
	}
	return 0;
}

void CFrontEnd::OnInitDialog( HWND hWndDlg )
{
}

void CFrontEnd::OnDrawItemButton( HWND hWndDlg, LPDRAWITEMSTRUCT pDIS )
{
	assert( hWndDlg && pDIS );

	char text[256];
	
	// *** Draw the button bitmap
	if( pDIS->itemState & ODS_SELECTED )
		m_bmpButtonDown.Draw( pDIS->hDC,0,0);
	else
		m_bmpButtonUp.Draw( pDIS->hDC,0,0);
	
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


HRESULT CFrontEnd::LaunchLoadGame( const u32 iSlot )
{
	// Launch our game using the specified slot!
	strcat( m_strCommandLine, LOADGAME_COMMAND_LINE_TOKEN );
	return LaunchGame( iSlot );
}

HRESULT CFrontEnd::LaunchNewGame( const u32 iSlot )
{
	// Launch our game using the specified slot!
	strcat( m_strCommandLine, NEWGAME_COMMAND_LINE_TOKEN );
	return LaunchGame( iSlot );
}

HRESULT CFrontEnd::LaunchGame( const u32 iSlot )
{
	if( iSlot > NUM_SAVE_SLOTS ) { ErrorMessage("Invalid Slot Number: %d !", iSlot ); return E_INVALIDARG; } 
	
	HINSTANCE	hInstance;
	char		strParameters[512];
	char		strSlotToken[16];
	
	// --- Setup our parameter string
	sprintf(strSlotToken, "-SAVESLOT=%d", iSlot );
	sprintf(strParameters, "%s %s", m_strCommandLine, strSlotToken );
	
	// Call ShellExecute to launch the game
	hInstance = ShellExecute(	m_hWnd,							// HWND hwnd, 
								"open",							// LPCTSTR lpVerb, (edit, find, open, print, properties)
								DEFAULT_LAUNCHAPP_FILENAME,		// LPCTSTR lpFile, 
								strParameters,					// LPCTSTR lpParameters, 
								Util_GetBaseDir( m_hInstance ),	// LPCTSTR lpDirectory,
								SW_SHOWNORMAL );				// INT nShowCmd
	
	if( hInstance == NULL )
	{
		MessageBox(m_hWnd, "Error Launching Application!", "CFrontEnd::Launch() Error", MB_OK );
		return E_FAIL;
	}

	// The FrontEnd's job is done, so lets quit our application
	PostQuitMessage( 0 );
	return S_OK;
}

void CFrontEnd::EmptySaveSlotFiles( const u32 iSlot )
{
	if( iSlot > NUM_SAVE_SLOTS ) { ErrorMessage("Invalid Slot Number: %d !", iSlot ); return; } 
	
	char		Dir[1024];
	char		Path[1024];
	
	// get the path to this saveSlot
	strcpy( Dir, m_SaveSlots[iSlot].m_Path );
	strstr( Dir, DEFAULT_SAVE_FILENAME )[0] = 0;
	
	strcpy( Path, Dir );
	strcat( Path, "*.*");
	
	// Delete all files in the path
	WIN32_FIND_DATAA Data;
	HANDLE Handle = NULL;
	Handle = FindFirstFileA( Path, &Data );
	bool Directories = false;
	bool Files = true;
	if( Handle != INVALID_HANDLE_VALUE )
	{	
		do
		{
			if(	stricmp( Data.cFileName, TEXT(".") ) && 
				stricmp( Data.cFileName, TEXT("..")) &&	
				((Data.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY)?Directories:Files) )
			{
				// Construct the correct Path to the file we are going to delete
				strcpy( Path, Dir );
				strcat( Path, Data.cFileName );
				
				// Delete this file
				if( !Util_FileDelete( Path , false, true ) )
					MessageBox(m_hWnd, Path, "Error Deleting file!", MB_OK );
			}
		}
		while( FindNextFileA(Handle, &Data) );
	}
	
	if( Handle )
		FindClose( Handle );
	
	// Set the empty boolean for this slot
	m_SaveSlots[iSlot].m_bEmpty = true;
}



// -----------------------------------------------------------------------------
// FrontEnd.cpp - End of file
// -----------------------------------------------------------------------------