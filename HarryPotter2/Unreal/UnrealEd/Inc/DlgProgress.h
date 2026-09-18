/*=============================================================================
	Progress : Progress indicator
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:
	- cancel button needs to work

=============================================================================*/

#include <stdio.h>

class WDlgProgress : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgProgress,WDialog,UnrealEd)

	WButton CancelButton;

	// Variables.

	// Constructor.
	WDlgProgress( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog				( TEXT("Progress"), IDDIALOG_PROGRESS, InOwnerWindow )
	,	CancelButton		( this, IDPB_CANCEL, FDelegate(this,(TDelegate)OnCancel) )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgProgress::OnInitDialog);
		WDialog::OnInitDialog();
		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgProgress::OnDestroy);
		WDialog::OnDestroy();
		::DestroyWindow( hWnd );
		unguard;
	}
	virtual void DoModeless()
	{
		guard(WDlgProgress::DoModeless);
		_Windows.AddItem( this );
		hWnd = CreateDialogParamA( hInstance, MAKEINTRESOURCEA(IDDIALOG_PROGRESS), OwnerWindow?OwnerWindow->hWnd:NULL, (DLGPROC)StaticDlgProc, (LPARAM)this);
		if( !hWnd )
			appGetLastError();
		unguard;
	}
	void OnCancel()
	{
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
