/*=============================================================================
	MapImport : Options for importing maps
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include <stdio.h>

class WDlgMapImport : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgMapImport,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WCheckBox NewMapCheck;

	FString Filename;
	UBOOL bNewMapCheck;

	// Constructor.
	WDlgMapImport( WWindow* InOwnerWindow )
	:	WDialog			( TEXT("Map Import"), IDDIALOG_IMPORT_MAP, InOwnerWindow )
	,	OkButton		( this, IDOK,			FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton	( this, IDCANCEL,		FDelegate(this,(TDelegate)EndDialogFalse) )
	,	NewMapCheck		( this, IDCK_NEW_MAP)
	{
		bNewMapCheck = 0;
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgMapImport::OnInitDialog);
		WDialog::OnInitDialog();
		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgMapImport::OnDestroy);
		WDialog::OnDestroy();
		::DestroyWindow( hWnd );
		unguard;
	}
	virtual int DoModal( FString _Filename )
	{
		guard(WDlgMapImport::DoModal);
		Filename = _Filename;
		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgMapImport::OnOk);
		bNewMapCheck = NewMapCheck.IsChecked();
		EndDialog(1);
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
