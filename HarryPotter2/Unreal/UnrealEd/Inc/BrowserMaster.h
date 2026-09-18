/*=============================================================================
	BrowserMaster : Master window where all "docked" browser reside
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include <stdio.h>

// --------------------------------------------------------------
//
// WBrowserMaster
//
// --------------------------------------------------------------

class WBrowserMaster : public WBrowser
{
	DECLARE_WINDOWCLASS(WBrowserMaster,WBrowser,Window)

	WToolTip* ToolTipCtrl;
	WBrowser** Browsers[eBROWSER_MAX];
	WTabControl* BrowserTabs;
	int CurrentBrowser;

	// Structors.
	WBrowserMaster( FName InPersistentName, WWindow* InOwnerWindow )
	:	WBrowser( InPersistentName, InOwnerWindow, NULL )
	{
		for( int x = 0 ; x < eBROWSER_MAX ; x++ )
			Browsers[x] = NULL;
		CurrentBrowser = -1;
		BrowserTabs = NULL;
		Description = TEXT("Browsers");
	}

	// WBrowser interface.
	void OpenWindow( UBOOL bChild )
	{
		guard(WBrowserMaster::OpenWindow);
		WBrowser::OpenWindow( bChild );
		Show(1);
		SetCaption();
		unguard;
	}
	void ShowBrowser( int InBrowser )
	{
		guard(WBrowserMaster::ShowBrowser);

		check(Browsers[InBrowser]);
		check(*Browsers[InBrowser]);

		CurrentBrowser = InBrowser;

		(*Browsers[InBrowser])->Show(1);
		::BringWindowToTop( (*Browsers[InBrowser])->hWnd );

		if( (*Browsers[InBrowser])->IsDocked() )
		{
			UpdateBrowserPositions();
			RefreshBrowserTabs( InBrowser );
			(*Browsers[InBrowser])->SetCaption();
			Show(1);
		}

		unguard;
	}
	void UpdateBrowserPositions()
	{
		guard(WBrowserMaster::UpdateBrowserPositions);

		RECT rect;
		::GetClientRect( hWnd, &rect );

		for( int x = 0 ; x < eBROWSER_MAX ; x++ )
		{
			// If the browser is docked, we need to fit it inside of our client area.
			if( Browsers[x] && *Browsers[x] && (*Browsers[x])->IsDocked() )
				::MoveWindow( (*Browsers[x])->hWnd, 4, 32, rect.right - 8 , rect.bottom - 36, 1);
		}
		unguard;
	}
	void OnCreate()
	{
		guard(WBrowserMaster::OnCreate);
		WBrowser::OnCreate();

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();

		BrowserTabs = new WTabControl( this, IDCB_BROWSER );
		BrowserTabs->OpenWindow( 1 );
		BrowserTabs->SelectionChangeDelegate = FDelegate(this, (TDelegate)OnBrowserTabSelChange);

		PositionChildControls();

		unguard;
	}
	void OnDestroy()
	{
		guard(WBrowserMaster::OnDestroy);
		WBrowser::OnDestroy();
		delete ToolTipCtrl;
		delete BrowserTabs;
		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WBrowserMaster::OnSize);
		WBrowser::OnSize(Flags, NewX, NewY);
		PositionChildControls();
		UpdateBrowserPositions();
		InvalidateRect( hWnd, NULL, 1 );
		unguard;
	}
	void PositionChildControls( void )
	{
		guard(WBrowserMaster::PositionChildControls);

		if( !BrowserTabs ) return;

		RECT rect;
		::GetClientRect( *this, &rect );
		::MoveWindow( BrowserTabs->hWnd, 0, 0, rect.right, rect.bottom, 1 );

		::InvalidateRect( hWnd, NULL, 1);

		unguard;
	}
	// Check to see how many browsers are docked and create buttons for them.
	void RefreshBrowserTabs( int InBrowser )
	{
		guard(WBrowserMaster::RefreshBrowserTabs);

		BrowserTabs->Empty();

		for( int x = 0 ; x < eBROWSER_MAX ; x++ )
			if( Browsers[x] && *Browsers[x] && (*Browsers[x])->IsDocked() )
				BrowserTabs->AddTab( *(*Browsers[x])->Description, (*Browsers[x])->BrowserID );

		if( !BrowserTabs->GetCount() )
		{
			HMENU CurrentMenu = GetMenu( hWnd );
			DestroyMenu( CurrentMenu );
			SetMenu( hWnd, NULL );
		}
		else
		{
			if( InBrowser != -1 )
			{
				if( Browsers[InBrowser] && *Browsers[InBrowser] && (*Browsers[InBrowser])->IsDocked() )
					BrowserTabs->SetCurrent( BrowserTabs->GetIndexFromlParam( (*Browsers[InBrowser])->BrowserID ) );

				HMENU CurrentMenu = GetMenu( hWnd );
				DestroyMenu( CurrentMenu );
				SetMenu( hWnd, LoadMenuIdX(hInstance, (*Browsers[InBrowser])->MenuID) );
				(*Browsers[InBrowser])->UpdateMenu();
			}
			else
			{
				BrowserTabs->SetCurrent(0);
				InBrowser = FindBrowserIdxFromName( BrowserTabs->GetString(0) );
				ShowBrowser( InBrowser );
				return;
			}
		}

		unguard;
	}
	virtual void RefreshAll()
	{
		guard(WBrowserMaster::RefreshAll);
		for( int x = 0 ; x < eBROWSER_MAX ; x++ )
			if( Browsers[x] && *Browsers[x] )
				(*Browsers[x])->RefreshAll();
		unguard;
	}
	int FindBrowserIdxFromName( FString InDesc )
	{
		guard(WBrowserMaster::FindBrowserIdxFromName);
		for( int x = 0 ; x < eBROWSER_MAX ; x++ )
			if( Browsers[x] && *Browsers[x] && (*Browsers[x])->Description == InDesc )
				return x;

		return 0;
		unguard;
	}
	void OnCommand( INT Command )
	{
		guard(WBrowserMaster::OnCommand);
		// If we don't want to deal with this message, pass it to the currently active browser.
		if( CurrentBrowser > -1 && Browsers[CurrentBrowser] && (*Browsers[CurrentBrowser])->IsDocked() )
			SendMessageX( (*Browsers[CurrentBrowser])->hWnd, WM_COMMAND, Command, 0);
		else
			WBrowser::OnCommand(Command);
		unguard;
	}
	int GetCurrent()
	{
		guard(WBrowserMaster::GetCurrent);
		return CurrentBrowser;
		unguard;
	}
	void OnBrowserTabSelChange()
	{
		guard(WBrowserMaster::OnBrowserTabSelChange);
		ShowBrowser( BrowserTabs->GetlParam( BrowserTabs->GetCurrent() ) );
		unguard;
	}

};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
