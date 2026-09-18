/*=============================================================================
	BrowserActor : Browser window for actor classes
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include <stdio.h>

// --------------------------------------------------------------
//
// NEW CLASS Dialog
//
// --------------------------------------------------------------

class WDlgNewClass : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgNewClass,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WLabel ParentLabel;
	WEdit PackageEdit;
	WEdit NameEdit;

	FString ParentClass, Package, Name;

	// Constructor.
	WDlgNewClass( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog			( TEXT("New Class"), IDDIALOG_NEW_CLASS, InOwnerWindow )
	,	OkButton		( this, IDOK,			FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton	( this, IDCANCEL,		FDelegate(this,(TDelegate)EndDialogFalse) )
	,	ParentLabel		( this, IDSC_PARENT )
	,	PackageEdit		( this, IDEC_PACKAGE )
	,	NameEdit		( this, IDEC_NAME )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgNewClass::OnInitDialog);
		WDialog::OnInitDialog();

		ParentLabel.SetText( *ParentClass );
		PackageEdit.SetText( *Package );
		NameEdit.SetText( *(FString::Printf(TEXT("My%s"), *ParentClass) ) );
		::SetFocus( PackageEdit.hWnd );

		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgNewClass::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	virtual int DoModal( FString _ParentClass, FString _ParentPackage )
	{
		guard(WDlgNewClass::DoModal);

		ParentClass = _ParentClass;
		Package = _ParentPackage;

		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgNewClass::OnOk);
		if( GetDataFromUser() )
		{
			// Check if class already exists.
			//

			// Create new class.
			//
			GEditor->Exec( *(FString::Printf( TEXT("CLASS NEW NAME=\"%s\" PACKAGE=\"%s\" PARENT=\"%s\""),
				*Name, *Package, *ParentClass)) );
			GEditor->Exec( *(FString::Printf(TEXT("SETCURRENTCLASS CLASS=\"%s\""), *Name)) );

			// Create standard header for the new class.
			//
			char ch13 = '\x0d', ch10 = '\x0a';
			FString S = FString::Printf(
				TEXT("//=============================================================================%c%c// %s.%c%c//=============================================================================%c%cclass %s expands %s;%c%c"),
				ch13, ch10,
				*Name, ch13, ch10,
				ch13, ch10,
				*Name, *ParentClass, ch13, ch10);
			GEditor->Set(TEXT("SCRIPT"), *Name, *S);

			EndDialog(TRUE);
		}
		unguard;
	}
	BOOL GetDataFromUser( void )
	{
		guard(WDlgNewClass::GetDataFromUser);
		Package = PackageEdit.GetText();
		Name = NameEdit.GetText();

		if( !Package.Len()
				|| !Name.Len() )
		{
			appMsgf( TEXT("Invalid input.") );
			return FALSE;
		}
		else
			return TRUE;
		unguard;
	}
};

// --------------------------------------------------------------
//
// WBrowserActor
//
// --------------------------------------------------------------

#define ID_BA_TOOLBAR	29030
TBBUTTON tbBAButtons[] = {
	{ 0, IDMN_MB_DOCK, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 1, IDMN_AB_FileOpen, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 2, IDMN_AB_FileSave, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 3, IDMN_AB_NEW_CLASS, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 4, IDMN_AB_EDIT_SCRIPT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 5, IDMN_AB_DEF_PROP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
};
struct {
	TCHAR ToolTip[64];
	int ID;
} ToolTips_BA[] = {
	TEXT("Toggle Dock Status"), IDMN_MB_DOCK,
	TEXT("Open Package"), IDMN_AB_FileOpen,
	TEXT("Save Selected Packages"), IDMN_AB_FileSave,
	TEXT("New Script"), IDMN_AB_NEW_CLASS,
	TEXT("Edit Script"), IDMN_AB_EDIT_SCRIPT,
	TEXT("Edit Default Properties"), IDMN_AB_DEF_PROP,
	NULL, 0
};

class WBrowserActor : public WBrowser
{
	DECLARE_WINDOWCLASS(WBrowserActor,WBrowser,Window)

	WTreeView* pTreeView;
	WCheckBox* pObjectsCheck;
	WCheckListBox* pPackagesList;
	HTREEITEM htiRoot, htiLastSel;
	HWND hWndToolBar;
	WToolTip* ToolTipCtrl;

	UBOOL bShowPackages;

	// Structors.
	WBrowserActor( FName InPersistentName, WWindow* InOwnerWindow, HWND InEditorFrame )
	:	WBrowser( InPersistentName, InOwnerWindow, InEditorFrame )
	{
		pTreeView = NULL;
		pObjectsCheck = NULL;
		htiRoot = htiLastSel = NULL;
		MenuID = IDMENU_BrowserActor;
		BrowserID = eBROWSER_ACTOR;
		Description = TEXT("Actor Classes");
	}

	// WBrowser interface.
	void OpenWindow( UBOOL bChild )
	{
		guard(WBrowserActor::OpenWindow);
		WBrowser::OpenWindow( bChild );
		SetCaption();
		unguard;
	}
	virtual void SetCaption( FString* Tail = NULL )
	{
		guard(WBrowserActor::SetCaption);

		FString Extra;
		if( GEditor->CurrentClass )
		{
			Extra = GEditor->CurrentClass->GetFullName();
			Extra = Extra.Right( Extra.Len() - 6 );	// remove "class" from the front of it
		}

		WBrowser::SetCaption( &Extra );
		unguard;
	}
	void OnCreate()
	{
		guard(WBrowserActor::OnCreate);
		WBrowser::OnCreate();

		SetMenu( hWnd, LoadMenuIdX(hInstance, IDMENU_BrowserActor) );
		
		pObjectsCheck = new WCheckBox( this, IDCK_OBJECTS );
		pObjectsCheck->ClickDelegate = FDelegate(this, (TDelegate)OnObjectsClick);
		pObjectsCheck->OpenWindow( 1, 0, 0, 1, 1, TEXT("Actor classes only") );
		SendMessageX( pObjectsCheck->hWnd, BM_SETCHECK, BST_CHECKED, 0 );

		pTreeView = new WTreeView( this, IDTV_TREEVIEW );
		pTreeView->OpenWindow( 1, 1, 0, 0, 1 );
		pTreeView->SelChangedDelegate = FDelegate(this, (TDelegate)OnTreeViewSelChanged);
		pTreeView->ItemExpandingDelegate = FDelegate(this, (TDelegate)OnTreeViewItemExpanding);
		pTreeView->DblClkDelegate = FDelegate(this, (TDelegate)OnTreeViewDblClk);
		
		pPackagesList = new WCheckListBox( this, IDLB_PACKAGES );
		pPackagesList->OpenWindow( 1, 0, 0, 1 );

		if(!GConfig->GetInt( *PersistentName, TEXT("ShowPackages"), bShowPackages, TEXT("UnrealEd.ini") ))		bShowPackages = 1;
		UpdateMenu();

		hWndToolBar = CreateToolbarEx( 
			hWnd, WS_CHILD | WS_BORDER | WS_VISIBLE | CCS_ADJUSTABLE,
			IDB_BrowserActor_TOOLBAR,
			6,
			hInstance,
			IDB_BrowserActor_TOOLBAR,
			(LPCTBBUTTON)&tbBAButtons,
			8,
			16,16,
			16,16,
			sizeof(TBBUTTON));
		check(hWndToolBar);

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();
		for( int tooltip = 0 ; ToolTips_BA[tooltip].ID > 0 ; tooltip++ )
		{
			// Figure out the rectangle for the toolbar button.
			int index = SendMessageX( hWndToolBar, TB_COMMANDTOINDEX, ToolTips_BA[tooltip].ID, 0 );
			RECT rect;
			SendMessageX( hWndToolBar, TB_GETITEMRECT, index, (LPARAM)&rect);

			ToolTipCtrl->AddTool( hWndToolBar, ToolTips_BA[tooltip].ToolTip, tooltip, &rect );
		}

		PositionChildControls();
		RefreshPackages();
		RefreshActorList();
		SendMessageX( pTreeView->hWnd, TVM_EXPAND, TVE_EXPAND, (LPARAM)htiRoot );

		unguard;
	}
	virtual void UpdateMenu()
	{
		guard(WBrowserActor::UpdateMenu);

		HMENU menu = IsDocked() ? GetMenu( OwnerWindow->hWnd ) : GetMenu( hWnd );

		CheckMenuItem( menu, IDMN_AB_SHOWPACKAGES, MF_BYCOMMAND | (bShowPackages ? MF_CHECKED : MF_UNCHECKED) );
		CheckMenuItem( menu, IDMN_MB_DOCK, MF_BYCOMMAND | (IsDocked() ? MF_CHECKED : MF_UNCHECKED) );

		unguard;
	}
	void RefreshPackages(void)
	{
		guard(WBrowserActor::RefreshPackages);

		// PACKAGES
		//
		FStringOutputDevice GetPropResult = FStringOutputDevice();
	    GEditor->Get(TEXT("OBJ"), TEXT("PACKAGES CLASS=Class"), GetPropResult);

		TArray<FString> PkgArray;
		ParseStringToArray( TEXT(","), GetPropResult, &PkgArray );

		pPackagesList->Empty();

		for( int x = 0 ; x < PkgArray.Num() ; x++ )
			pPackagesList->AddString( *(FString::Printf( TEXT("%s"), *PkgArray(x))) );

		unguard;
	}
	void OnDestroy()
	{
		guard(WBrowserActor::OnDestroy);

		delete pTreeView;
		delete pObjectsCheck;
		delete pPackagesList;

		::DestroyWindow( hWndToolBar );
		delete ToolTipCtrl;

		GConfig->SetInt( *PersistentName, TEXT("ShowPackages"), bShowPackages, TEXT("UnrealEd.ini") );

		WBrowser::OnDestroy();
		unguard;
	}
	void OnCommand( INT Command )
	{
		guard(WBrowserActor::OnCommand);
		switch( Command )
		{
			case WM_TREEVIEW_RIGHT_CLICK:
				{
					// Select the tree item underneath the mouse cursor.
					TVHITTESTINFO tvhti;
					POINT ptScreen;
					::GetCursorPos( &ptScreen );
					tvhti.pt = ptScreen;
					::ScreenToClient( pTreeView->hWnd, &tvhti.pt );

					SendMessageX( pTreeView->hWnd, TVM_HITTEST, 0, (LPARAM)&tvhti);

					if( tvhti.hItem )
						SendMessageX( pTreeView->hWnd, TVM_SELECTITEM, TVGN_CARET, (LPARAM)(HTREEITEM)tvhti.hItem);

					// Show a context menu for the currently selected item.
					HMENU menu = GetSubMenu( LoadMenuIdX(hInstance, IDMENU_BrowserActor_Context), 0 );
					TrackPopupMenu( menu,
						TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
						ptScreen.x, ptScreen.y, 0,
						hWnd, NULL);
				}
				break;

			case IDMN_AB_EXPORT_ALL:
				{
					if( ::MessageBox( hWnd, TEXT("This option will export all classes to text .uc files which can later be rebuilt. Do you want to do this?"), TEXT("Export classes to *.uc files"), MB_YESNO) == IDYES)
					{
						GEditor->Exec( TEXT("CLASS SPEW ALL") );
					}
				}
				break;

			case IDMN_AB_EXPORT:
				{
					if( ::MessageBox( hWnd, TEXT("This option will export all modified classes to text .uc files which can later be rebuilt. Do you want to do this?"), TEXT("Export classes to *.uc files"), MB_YESNO) == IDYES)
					{
						GEditor->Exec( TEXT("CLASS SPEW") );
					}
				}
				break;

			case IDMN_AB_SHOWPACKAGES:
				{
					bShowPackages = !bShowPackages;
					PositionChildControls();
					UpdateMenu();
				}
				break;

			case IDMN_AB_NEW_CLASS:
				{
					WDlgNewClass l_dlg( NULL, this );
					if( l_dlg.DoModal( 
						GEditor->CurrentClass ? GEditor->CurrentClass->GetName() : TEXT("Actor"),
						GEditor->CurrentClass ? GEditor->CurrentClass->GetOuter()->GetPathName() : TEXT("Engine")
					) )
					{
						// Open an editing window.
						//
						GCodeFrame->AddClass( GEditor->CurrentClass );
						RefreshActorList();
						RefreshPackages();
					}
				}
				break;

			case IDMN_AB_DELETE:
				{
					if( GEditor->CurrentClass )
					{
						FString CurName = GEditor->CurrentClass->GetName();
						GEditor->Exec( TEXT("SETCURRENTCLASS Class=Light") );

						TCHAR l_chCmd[256];
						FStringOutputDevice GetPropResult = FStringOutputDevice();
						appSprintf( l_chCmd, TEXT("DELETE CLASS=CLASS OBJECT=\"%s\""), *CurName );
						
						GEditor->Get( TEXT("OBJ"), l_chCmd, GetPropResult);

						if( !GetPropResult.Len() )
						{
							// Try to cleanly update the actor list.  If this fails, just reload it from scratch...
							if( !SendMessageX( pTreeView->hWnd, TVM_DELETEITEM, 0, (LPARAM)htiLastSel ) )
								RefreshActorList();

							GCodeFrame->RemoveClass( CurName );
						}
						else
							appMsgf( TEXT("Can't delete: %s"), *GetPropResult );
					}
				}
				break;

			case IDMN_AB_DEF_PROP:
				GEditor->Exec( *(FString::Printf(TEXT("HOOK CLASSPROPERTIES CLASS=\"%s\""), GEditor->CurrentClass->GetName())) );
				break;

			case IDMN_AB_RESET_PROP:
				GEditor->Exec( *(FString::Printf(TEXT("ACTOR SETTOCLASSDEFAULT CLASS=\"%s\""), GEditor->CurrentClass->GetName())) );
				break;
			
			case IDMN_AB_FileOpen:
				{
					OPENFILENAMEA ofn;
					char File[8192] = "\0";

					ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
					ofn.lStructSize = sizeof(OPENFILENAMEA);
					ofn.hwndOwner = hWnd;
					ofn.lpstrFile = File;
					ofn.nMaxFile = sizeof(char) * 8192;
					ofn.lpstrFilter = "Class Packages (*.u)\0*.u\0All Files\0*.*\0\0";
					ofn.lpstrInitialDir = "..\\system";
					ofn.lpstrDefExt = "u";
					ofn.lpstrTitle = "Open Class Package";
					ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_ALLOWMULTISELECT | OFN_EXPLORER;

					if( GetOpenFileNameA(&ofn) )
					{
						int iNULLs = FormatFilenames( File );
		
						TArray<FString> StringArray;
						ParseStringToArray( TEXT("|"), appFromAnsi( File ), &StringArray );

						int iStart = 0;
						FString Prefix = TEXT("\0");

						if( iNULLs )
						{
							iStart = 1;
							Prefix = *(StringArray(0));
							Prefix += TEXT("\\");
						}

						for( int x = iStart ; x < StringArray.Num() ; x++ )
						{
							TCHAR l_chCmd[512];

							appSprintf( l_chCmd, TEXT("CLASS LOAD FILE=\"%s%s\""), *Prefix, *(StringArray(x)) );
							GEditor->Exec( l_chCmd );
						}

						GBrowserMaster->RefreshAll();
						RefreshPackages();
					}

					GFileManager->SetDefaultDirectory(appBaseDir());
					RefreshPackages();
				}
				break;

			case IDMN_AB_FileSave:
				{
					FString Pkg;

					GWarn->BeginSlowTask( TEXT("Saving Packages"), 1, 0 );

					for( int x = 0 ; x < pPackagesList->GetCount() ; x++ )
					{
						if( (int)pPackagesList->GetItemData(x) )
						{
							Pkg = *(pPackagesList->GetString( x ));
							GEditor->Exec( *(FString::Printf(TEXT("OBJ SAVEPACKAGE PACKAGE=\"%s\" FILE=\"%s.u\""), *Pkg, *Pkg )) );
						}
					}

					GWarn->EndSlowTask();
				}
				break;

			case IDMN_AB_EDIT_SCRIPT:
				{
					GCodeFrame->AddClass( GEditor->CurrentClass );
				}
				break;

			default:
				WBrowser::OnCommand(Command);
				break;
		}
		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WBrowserActor::OnSize);
		WBrowser::OnSize(Flags, NewX, NewY);
		PositionChildControls();
		InvalidateRect( hWnd, NULL, FALSE );
		UpdateMenu();
		unguard;
	}
	void PositionChildControls( void )
	{
		guard(WBrowserActor::PositionChildControls);

		if( !pTreeView
				|| !pObjectsCheck
				|| !pPackagesList ) return;

		FRect CR = GetClientRect();
		RECT R;
		::GetClientRect( hWndToolBar, &R );
		float Fraction = (CR.Width() - 8) / 10.0f;
		float Top = 4 + R.bottom;

		::MoveWindow( pObjectsCheck->hWnd, 4, Top, Fraction * 10, 20, 1 );		Top += 20;
		if( bShowPackages )
		{
			::MoveWindow( pTreeView->hWnd, 4, Top, CR.Width() - 8, ((CR.Height() / 3) * 2) - Top, 1 );	Top += ((CR.Height() / 3) * 2) - Top;
			::MoveWindow( pPackagesList->hWnd, 4, Top, CR.Width() - 8, (CR.Height() / 3) - 4, 1);
			debugf(TEXT("%1.2f %1.2f %1.2f %1.2f"),
				(float)4,
				(float)(Top),
				(float)(CR.Width() - 8),
				(float)((CR.Height() / 3) - 4 ));
		}
		else
		{
			::MoveWindow( pTreeView->hWnd, 4, Top, CR.Width() - 8, CR.Height() - Top - 4, 1 );
			::MoveWindow( pPackagesList->hWnd, 0, 0, 0, 0, 1);
		}

		::InvalidateRect( hWnd, NULL, 1);

		unguard;
	}
	virtual void RefreshAll()
	{
		guard(WBrowserActor::RefreshAll);
		RefreshActorList();
		unguard;
	}
	void RefreshActorList( void )
	{
		guard(WBrowserActor::RefreshActorList);
		pTreeView->Empty();

		if( pObjectsCheck->IsChecked() )
			htiRoot = pTreeView->AddItem( TEXT("Actor"), NULL, TRUE );
		else
			htiRoot = pTreeView->AddItem( TEXT("Object"), NULL, TRUE );

		htiLastSel = NULL;
		unguard;
	}
	void AddChildren( const TCHAR* pParentName, HTREEITEM hti )
	{
		guard(WBrowserActor::AddChildren);
		HTREEITEM newhti;
		FString String, StringQuery;

		StringQuery = FString::Printf( TEXT("Query Parent=\"%s\""), pParentName );
		Query( GEditor->Level, *StringQuery, &String );

		TArray<FString> StringArray;
		ParseStringToArray( TEXT(","), String, &StringArray );

		for( int x = 0 ; x < StringArray.Num() ; x++ )
		{
			FString NewName = *(StringArray(x)), Children;

			Children = NewName.Left(1);
			NewName = NewName.Right( NewName.Len() - 1 );

			newhti = pTreeView->AddItem( *NewName, hti, Children == TEXT("C") ? TRUE : FALSE );
		}
		unguard;
	}
	void OnTreeViewSelChanged( void )
	{
		guard(WBrowserActor::OnTreeViewSelChanged);
		NMTREEVIEW* pnmtv = (LPNMTREEVIEW)pTreeView->LastlParam;
		TCHAR chText[128] = TEXT("\0");
		TVITEM tvi;

		appMemzero( &tvi, sizeof(tvi));
		htiLastSel = tvi.hItem = pnmtv->itemNew.hItem;
		tvi.mask = TVIF_TEXT;
		tvi.pszText = chText;
		tvi.cchTextMax = sizeof(chText);

		if( SendMessageX( pTreeView->hWnd, TVM_GETITEM, 0, (LPARAM)&tvi) )
			GEditor->Exec( *(FString::Printf(TEXT("SETCURRENTCLASS CLASS=\"%s\""), tvi.pszText )));
		SetCaption();
		unguard;
	}
	void OnTreeViewItemExpanding( void )
	{
		guard(WBrowserActor::OnTreeViewItemExpanding);
		NMTREEVIEW* pnmtv = (LPNMTREEVIEW)pTreeView->LastlParam;
		TCHAR chText[128] = TEXT("\0");

		TVITEM tvi;

		appMemzero( &tvi, sizeof(tvi));
		tvi.hItem = pnmtv->itemNew.hItem;
		tvi.mask = TVIF_TEXT;
		tvi.pszText = chText;
		tvi.cchTextMax = sizeof(chText);

		// If this item already has children loaded, bail...
		if( SendMessageX( pTreeView->hWnd, TVM_GETNEXTITEM, TVGN_CHILD, (LPARAM)pnmtv->itemNew.hItem ) )
			return;

		if( SendMessageX( pTreeView->hWnd, TVM_GETITEM, 0, (LPARAM)&tvi) )
			AddChildren( tvi.pszText, pnmtv->itemNew.hItem );
		unguard;
	}
	void OnTreeViewDblClk( void )
	{
		guard(WBrowserActor::OnTreeViewDblClk);
		GCodeFrame->AddClass( GEditor->CurrentClass );
		unguard;
	}
	void OnObjectsClick()
	{
		guard(WBrowserActor::OnObjectsClick);
		RefreshActorList();
		SendMessageX( pTreeView->hWnd, TVM_EXPAND, TVE_EXPAND, (LPARAM)htiRoot );
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
