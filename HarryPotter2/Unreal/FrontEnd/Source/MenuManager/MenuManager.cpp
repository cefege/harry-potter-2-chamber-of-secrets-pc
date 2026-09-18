// -----------------------------------------------------------------------------
//  __  __                  __  __                                                        
// |  \/  |                |  \/  |                                                       
// | \  / | ___ _ __  _   _| \  / | __ _ _ __   __ _  __ _  ___ _ __      ___ _ __  _ __  
// | |\/| |/ _ \ '_ \| | | | |\/| |/ _` | '_ \ / _` |/ _` |/ _ \ '__|    / __| '_ \| '_ \ 
// | |  | |  __/ | | | |_| | |  | | (_| | | | | (_| | (_| |  __/ |    _ | (__| |_) | |_) |
// |_|  |_|\___|_| |_|\__,_|_|  |_|\__,_|_| |_|\__,_|\__, |\___|_|   (_) \___| .__/| .__/ 
//                                                    __/ |                  | |   | |    
//                                                   |___/                   |_|   |_|    
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002 by Elijah Emerson
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#define WIN32_LEAN_AND_MEAN
#include <windows.h>				// 
#include <mmsystem.h>				// timeGetTime()

#include <stack>
#include <assert.h>

#include "..\common\types.h"			
#include "..\common\win_utilities.h"	

#include ".\MenuManager.h"
#include ".\MenuManager_priv.h"

//----------------------------------------------------------------------------------
//	FUNCTION NAME:	CreateMenuManager
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	7/23/2002
//	
//	ARGUMENT 1:     HINSTANCE hInstance	- hanlde to module instance
//	RETURN TYPE:	IMenuManager* 		- (NULL) if fail, (menu interface) if successfull
//
//	DESCRIPTION:	Creates our menu manager interface
//----------------------------------------------------------------------------------
IMenuManager* CreateMenuManager( HINSTANCE hInstance )
{
	// Check arguments
	if( hInstance == 0 )
		return NULL;
	
	// Allocate memory for our MenuManager and initilize it
	CMenuManager* pManager = new CMenuManager;
	if( NULL == pManager )
		return NULL;
	
	if( FAILED( pManager->Init( hInstance )) )
	{
		SAFE_DELETE( pManager );
		return NULL;
	}
	return pManager;
}


// *************************************************************************************************************
// *** Private Functions

// --- Base Functions

void CMenuManager::Clear()
{
	// Clear all data
	m_hInstance			= NULL;
	m_hWnd				= NULL;
	
	m_bActive			= false;
	m_bMinimized		= false;
	m_bUpdateShown		= false;

	m_pLastMenu			= NULL;
}

HRESULT CMenuManager::Init( const HINSTANCE hInstance )
{
	if( hInstance == 0 )
		return E_INVALIDARG;
	
	// Clean out our memory
	Destroy();
	
	// Set the hInstance
	m_hInstance = hInstance;

	return S_OK;
}

void CMenuManager::Destroy( void )
{
	// Release all menus
	while( m_vMenuStack.size() )
	{
		DebugOutput( "MenuManager released hWnd = %d", ((IMenu*)m_vMenuStack.top())->GetHandle() );
		PopMenu();
	}
	Clear();
}

HRESULT CMenuManager::Release( void )
{
	delete this;
	return S_OK;
}

void CMenuManager::Update( void )
{
	// Time vars
	static float fTimePrevious;	
	static float fTimeCurrent = timeGetTime() * 0.001f;	
	static float fTimeDelta;

	// Compute fTimeDelta
	fTimePrevious	= fTimeCurrent;
	fTimeCurrent	= timeGetTime() * 0.001f;
	fTimeDelta		= fTimeCurrent - fTimePrevious;
	
	// cap our timeDelta
	if( fTimeDelta > 0.5 )
		fTimeDelta = 0.5f;

	// If we have a current menu then update it!
	if( m_vMenuStack.size() )
	{
		if( m_vMenuStack.top()->Update( fTimeDelta ) )
			PopMenu();	// This menu wants to pop
		else if( m_bUpdateShown )
		{
			m_bUpdateShown = false;
			SendMessage( m_vMenuStack.top()->GetHandle(), WM_MM_OPEN, 0, 0 );
			ShowWindow(  m_vMenuStack.top()->GetHandle(), SW_SHOW ); // show the topmost window
			InvalidateRect( m_vMenuStack.top()->GetHandle(), NULL, false );
		}
	}
}


IMenu* CMenuManager::PushMenu( const char* strName, const LPCTSTR lpTemplate, const DLGPROC dlgProc )
{
	// Check arguments
	if( dlgProc == NULL || strName == NULL || strName[0] == NULL)
		return NULL;
	
	// Allocate memory for our menu and initilize it
	CMenu* pMenu = new CMenu;
	if( NULL == pMenu )
		return NULL;

	// Create the new window as a child window of our current window (if we have one)
	HWND hWndMenu = CreateDialog( m_hInstance, 
								  lpTemplate, 
								  ( m_vMenuStack.size() ) ? m_vMenuStack.top()->GetHandle() : NULL, 
								  dlgProc );
	if( hWndMenu == NULL )
	{
		DisplayLastWinError();
		return NULL;
	}
	
	// init our new menu
	if( FAILED( pMenu->Init( strName, hWndMenu )) )
	{
		SAFE_DELETE( pMenu );
		return NULL;
	}
	
	// Set the default position of the menu in the middle of the screen
	pMenu->SetRelativePosition( 0.5f, 0.5f );

	// If we have a current menu already, save a refrence to it.	
	m_pLastMenu = ( m_vMenuStack.size() ) ? m_vMenuStack.top() : NULL;
	
	// Push our new menu onto the stack
	m_vMenuStack.push( pMenu );
	
	// this is later used to show the top window
	m_bUpdateShown = true;

	return pMenu;
}

// Pop the top most menu, return the current menu
IMenu* CMenuManager::PopMenu( void )
{
	if( m_vMenuStack.size() )
	{
		CMenu* pTop = m_vMenuStack.top();
		if( pTop == NULL )
			return NULL;
		
		// Send the default user message to let the end user know that we are about to close
		SendMessage( pTop->GetHandle(), WM_MM_CLOSE, 0, 0 );
		
		pTop->Release();		// Release the menu on the top of the stack
		m_vMenuStack.pop();		// Pop the menu off the top of the stack
		
		// this is later used to show the top window
		m_bUpdateShown = true;
		
		if( m_vMenuStack.size() )
			return m_vMenuStack.top();
	}
	return NULL;
}

//***************************************************************************************************************

// --- Base Functions

void CMenu::Clear( void )
{
	// Clear all data
	m_strName[0]		= NULL;
	m_hWnd				= NULL;
	
	m_bUsePopTimer		= false;
	m_fPopTimer			= 0.0f;
}

HRESULT CMenu::Init( const char* strName, const HWND hWndMenu )
{
	// Check arguments
	if( strName == NULL || strName[0] == NULL || hWndMenu == NULL )
		return E_INVALIDARG;
	
	// Make sure our menu is ready for initilization
	Destroy();
	
	// Set the handle to this window
	m_hWnd = hWndMenu;
	strcpy( m_strName, strName );
	return S_OK;
}

void CMenu::Destroy( void )
{
	// Destroy this window
	EndDialog( m_hWnd, 0 );
	Clear();
}


HRESULT CMenu::Release( void )
{
	delete this;
	return S_OK;
}

bool CMenu::Update( float fTimeDelta )
{
	// Update our pop timer if this menu has one
	if( m_bUsePopTimer )
	{
		m_fPopTimer -= fTimeDelta;
		if( m_fPopTimer <= 0.0f )
			return true;
	}
	return false;
}

// --- Set Functions

// Set a time for when this menu should automaticly pop
void CMenu::SetPopTimer( float fWaitTime )
{
	if( fWaitTime < 0.0f )
		fWaitTime = 0.0f;

	m_bUsePopTimer = true;
	m_fPopTimer    = fWaitTime;
}

// Set the position of the menu relitive to the screen
void CMenu::SetRelativePosition( f32 fXPercent, f32 fYPercent )
{
	Util_OrientWindow( m_hWnd, fXPercent, fYPercent );
}

// -----------------------------------------------------------------------------
// MenuManager.cpp - End of file
// -----------------------------------------------------------------------------