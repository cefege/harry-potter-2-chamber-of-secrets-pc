/*=============================================================================
	BrowserSound : Browser window for sound effects
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include <stdio.h>
extern FString GLastDir[eLASTDIR_MAX];

// --------------------------------------------------------------
//
// IMPORT SOUND Dialog
//
// --------------------------------------------------------------

class WDlgImportSound : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgImportSound,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton OkAllButton;
	WButton SkipButton;
	WButton CancelButton;
	WLabel FilenameStatic;
	WEdit PackageEdit;
	WEdit GroupEdit;
	WEdit NameEdit;

	FString defPackage, defGroup;
	TArray<FString>* paFilenames;

	FString Package, Group, Name;
	BOOL bOKToAll;
	int iCurrentFilename;

	// Constructor.
	WDlgImportSound( UObject* InContext, WBrowser* InOwnerWindow )
	:	WDialog			( TEXT("Import Sound"), IDDIALOG_IMPORT_SOUND, InOwnerWindow )
	,	OkButton		( this, IDOK,			FDelegate(this,(TDelegate)OnOk) )
	,	OkAllButton		( this, IDPB_OKALL,		FDelegate(this,(TDelegate)OnOkAll) )
	,	SkipButton		( this, IDPB_SKIP,		FDelegate(this,(TDelegate)OnSkip) )
	,	CancelButton	( this, IDCANCEL,		FDelegate(this,(TDelegate)EndDialogFalse) )
	,	PackageEdit		( this, IDEC_PACKAGE )
	,	GroupEdit		( this, IDEC_GROUP )
	,	NameEdit		( this, IDEC_NAME )
	,	FilenameStatic	( this, IDSC_FILENAME )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgImportSound::OnInitDialog);
		WDialog::OnInitDialog();

		PackageEdit.SetText( *defPackage );
		GroupEdit.SetText( *defGroup );
		::SetFocus( NameEdit.hWnd );

		bOKToAll = FALSE;
		iCurrentFilename = -1;
		SetNextFilename();

		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgImportSound::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	virtual int DoModal( FString _defPackage, FString _defGroup, TArray<FString>* _paFilenames)
	{
		guard(WDlgImportSound::DoModal);

		defPackage = _defPackage;
		defGroup = _defGroup;
		paFilenames = _paFilenames;

		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgImportSound::OnOk);
		if( GetDataFromUser() )
		{
			ImportFile( (*paFilenames)(iCurrentFilename) );
			SetNextFilename();
		}
		unguard;
	}
	void OnOkAll()
	{
		guard(WDlgImportSound::OnOkAll);
		if( GetDataFromUser() )
		{
			ImportFile( (*paFilenames)(iCurrentFilename) );
			bOKToAll = TRUE;
			SetNextFilename();
		}
		unguard;
	}
	void OnSkip()
	{
		guard(WDlgImportSound::OnSkip);
		if( GetDataFromUser() )
			SetNextFilename();
		unguard;
	}
	void ImportTexture( void )
	{
		guard(WDlgImportSound::ImportTexture);
		unguard;
	}
	void RefreshName( void )
	{
		guard(WDlgImportSound::RefreshName);
		FilenameStatic.SetText( *(*paFilenames)(iCurrentFilename) );

		FString Name = GetFilenameOnly( (*paFilenames)(iCurrentFilename) );
		NameEdit.SetText( *Name );
		unguard;
	}
	BOOL GetDataFromUser( void )
	{
		guard(WDlgImportSound::GetDataFromUser);
		Package = PackageEdit.GetText();
		Group = GroupEdit.GetText();
		Name = NameEdit.GetText();

		if( !Package.Len()
				|| !Name.Len() )
		{
			appMsgf( TEXT("Invalid input.") );
			return FALSE;
		}

		return TRUE;
		unguard;
	}
	void SetNextFilename( void )
	{
		guard(WDlgImportSound::SetNextFilename);
		iCurrentFilename++;
		if( iCurrentFilename == paFilenames->Num() ) {
			EndDialogTrue();
			return;
		}

		if( bOKToAll ) {
			RefreshName();
			GetDataFromUser();
			ImportFile( (*paFilenames)(iCurrentFilename) );
			SetNextFilename();
			return;
		};

		RefreshName();

		unguard;
	}
	void ImportFile( FString Filename )
	{
		guard(WDlgImportSound::ImportFile);
		TCHAR l_chCmd[512];

		if( Group.Len() )
			appSprintf( l_chCmd, TEXT("AUDIO IMPORT FILE=\"%s\" NAME=\"%s\" PACKAGE=\"%s\" GROUP=\"%s\""),
				*(*paFilenames)(iCurrentFilename), *Name, *Package, *Group );
		else
			appSprintf( l_chCmd, TEXT("AUDIO IMPORT FILE=\"%s\" NAME=\"%s\" PACKAGE=\"%s\""),
				*(*paFilenames)(iCurrentFilename), *Name, *Package );
		GEditor->Exec( l_chCmd );
		unguard;
	}
};

// --------------------------------------------------------------
//
// WBROWSERSOUND
//
// --------------------------------------------------------------

#define ID_BS_TOOLBAR	29020
TBBUTTON tbBSButtons[] = {
	{ 0, IDMN_MB_DOCK, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 1, IDMN_SB_FileOpen, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 2, IDMN_SB_FileSave, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 3, IDMN_SB_PLAY, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 4, IDMN_SB_STOP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
};
struct {
	TCHAR ToolTip[64];
	int ID;
} ToolTips_BS[] = {
	TEXT("Toggle Dock Status"), IDMN_MB_DOCK,
	TEXT("Open Package"), IDMN_SB_FileOpen,
	TEXT("Save Package"), IDMN_SB_FileSave,
	TEXT("Play"), IDMN_SB_PLAY,
	TEXT("Stop"), IDMN_SB_STOP,
	NULL, 0
};

class WBrowserSound : public WBrowser
{
	DECLARE_WINDOWCLASS(WBrowserSound,WBrowser,Window)

	WComboBox *pComboPackage, *pComboGroup;
	WListBox *pListSounds;
	WCheckBox *pCheckGroupAll;
//	WCheckBox  *pCheckStreaming, *pCheckLooping;
	HWND hWndToolBar;
	WToolTip *ToolTipCtrl;
	MRUList* mrulist;
	WLabel * pSoundText, *pFileType, * pNumChannels, * pLength;

	// Structors.
	WBrowserSound( FName InPersistentName, WWindow* InOwnerWindow, HWND InEditorFrame )
	:	WBrowser( InPersistentName, InOwnerWindow, InEditorFrame )
	{
		pComboPackage = pComboGroup = NULL;
		pListSounds = NULL;
		pCheckGroupAll = NULL;
		MenuID = IDMENU_BrowserSound;
		BrowserID = eBROWSER_SOUND;
		Description = TEXT("Sounds");
	}

	// WBrowser interface.
	void OpenWindow( UBOOL bChild )
	{
		guard(WBrowserSound::OpenWindow);
		WBrowser::OpenWindow( bChild );
		SetCaption();
		unguard;
	}
	void OnCreate()
	{
		guard(WBrowserSound::OnCreate);
		WBrowser::OnCreate();

		SetMenu( hWnd, LoadMenuIdX(hInstance, IDMENU_BrowserSound) );

		// PACKAGES
		//
		pComboPackage = new WComboBox( this, IDCB_PACKAGE );
		pComboPackage->OpenWindow( 1, 1 );
		pComboPackage->SelectionChangeDelegate = FDelegate(this, (TDelegate)OnComboPackageSelChange);

		// GROUP
		//
		pComboGroup = new WComboBox( this, IDCB_GROUP );
		pComboGroup->OpenWindow( 1, 1 );
		pComboGroup->SelectionChangeDelegate = FDelegate(this, (TDelegate)OnComboGroupSelChange);

		// SOUND LIST
		//
		pListSounds = new WListBox( this, IDLB_SOUNDS );
		pListSounds->OpenWindow( 1, 0, 0, 0, 1 );
		pListSounds->DoubleClickDelegate = FDelegate(this, (TDelegate)OnListSoundsDblClick);
		pListSounds->SelectionChangeDelegate = FDelegate(this, (TDelegate)UpdateCurrentSoundDetails);

		// CHECK BOXES
		//
		pCheckGroupAll = new WCheckBox( this, IDCK_GRP_ALL, FDelegate(this, (TDelegate)OnGroupAllClick) );
		pCheckGroupAll->OpenWindow( 1, 0, 0, 1, 1, TEXT("All"), 1, 0, BS_PUSHLIKE );

//		pCheckLooping = new WCheckBox( this, NULL, FDelegate(this, (TDelegate)OnLoopingClick) );
//		pCheckLooping->OpenWindow( 1, 0 ,0, 1, 1, TEXT("Looping"));
//		pListSounds->ClickDelegate = FDelegate(this, (TDelegate) ... insert );

//		pCheckStreaming = new WCheckBox (this, NULL, FDelegate(this, (TDelegate)OnStreamingClick ) );
//		pCheckStreaming->OpenWindow ( 1, 0 ,0, 1, 1, TEXT("Streaming"));

		// Text controls

		pSoundText = new WLabel (this);
		pSoundText->OpenWindow (1, 0);
		pSoundText->SetText ( TEXT("Sample details:"));

		pFileType = new WLabel (this);
		pFileType->OpenWindow (1);

		pNumChannels = new WLabel (this);
		pNumChannels->OpenWindow (1);

		pLength = new WLabel (this);
		pLength->OpenWindow (1);


		hWndToolBar = CreateToolbarEx( 
			hWnd, WS_CHILD | WS_BORDER | WS_VISIBLE | CCS_ADJUSTABLE,
			IDB_BrowserSound_TOOLBAR,
			5,
			hInstance,
			IDB_BrowserSound_TOOLBAR,
			(LPCTBBUTTON)&tbBSButtons,
			7,
			16,16,
			16,16,
			sizeof(TBBUTTON));
		check(hWndToolBar);

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();
		for( int tooltip = 0 ; ToolTips_BS[tooltip].ID > 0 ; tooltip++ )
		{
			// Figure out the rectangle for the toolbar button.
			int index = SendMessageX( hWndToolBar, TB_COMMANDTOINDEX, ToolTips_BS[tooltip].ID, 0 );
			RECT rect;
			SendMessageX( hWndToolBar, TB_GETITEMRECT, index, (LPARAM)&rect);

			ToolTipCtrl->AddTool( hWndToolBar, ToolTips_BS[tooltip].ToolTip, tooltip, &rect );
		}

		mrulist = new MRUList( *PersistentName );
		mrulist->ReadINI();
		if( GBrowserMaster->GetCurrent()==BrowserID )
			mrulist->AddToMenu( hWnd, GetMenu( IsDocked() ? OwnerWindow->hWnd : hWnd ) );

		RefreshAll();
		PositionChildControls();

		unguard;
	}
	void OnDestroy()
	{
		guard(WBrowserSound::OnDestroy);

		mrulist->WriteINI();
		delete mrulist;

		delete pComboPackage;
		delete pComboGroup;
		delete pListSounds;
		delete pCheckGroupAll;
//		delete pCheckLooping;
//		delete pCheckStreaming;
		delete pSoundText;
		delete pFileType;
		delete pNumChannels;
		delete pLength;

		::DestroyWindow( hWndToolBar );
		delete ToolTipCtrl;

		WBrowser::OnDestroy();
		unguard;
	}
	virtual void UpdateMenu()
	{
		guard(WBrowserSound::UpdateMenu);

		HMENU menu = IsDocked() ? GetMenu( OwnerWindow->hWnd ) : GetMenu( hWnd );
		CheckMenuItem( menu, IDMN_MB_DOCK, MF_BYCOMMAND | (IsDocked() ? MF_CHECKED : MF_UNCHECKED) );

		if( mrulist
				&& GBrowserMaster->GetCurrent()==BrowserID )
			mrulist->AddToMenu( hWnd, GetMenu( IsDocked() ? OwnerWindow->hWnd : hWnd ) );

		unguard;
	}
	void OnCommand( INT Command )
	{
		guard(WBrowserSound::OnCommand);
		switch( Command ) {

			case IDMN_SB_DELETE:
				{
					FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );

					FString Group = pComboGroup->GetString( pComboGroup->GetCurrent () );

					FString Name = pListSounds->GetString( pListSounds->GetCurrent() );
					FStringOutputDevice GetPropResult = FStringOutputDevice();
					TCHAR l_chCmd[256];

					if (Group)
						appSprintf( l_chCmd, TEXT("DELETE CLASS=SOUND PACKAGE=\"%s\" OBJECT=\"%s.%s\""), *Package, *Group, *Name);
					else
						appSprintf( l_chCmd, TEXT("DELETE CLASS=SOUND PACKAGE=\"%s\" OBJECT=\"%s\""), *Package, *Name);

				    GEditor->Get( TEXT("Obj"), l_chCmd, GetPropResult);

					if( !GetPropResult.Len() )
						RefreshSoundList();
					else
						appMsgf( TEXT("Can't delete: %s"), *GetPropResult );
				}
				break;

			case IDMN_SB_EXPORT_WAV:
				{
					OPENFILENAMEA ofn;
					char File[8192] = "\0";
					FString Name = pListSounds->GetString( pListSounds->GetCurrent() );

					::sprintf( File, "%s", TCHAR_TO_ANSI( *Name ) );

					ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
					ofn.lStructSize = sizeof(OPENFILENAMEA);
					ofn.hwndOwner = hWnd;
					ofn.lpstrFile = File;
					ofn.nMaxFile = sizeof(char) * 8192;
					ofn.lpstrFilter = "WAV Files (*.wav)\0*.wav\0All Files\0*.*\0\0";
					ofn.lpstrDefExt = "wav";
					ofn.lpstrTitle = "Export Sound";
					ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_WAV]) );
					ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

					// Display the Open dialog box. 
					//
					if( GetSaveFileNameA(&ofn) )
					{
						TCHAR l_chCmd[512];
						FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );

						appSprintf( l_chCmd, TEXT("OBJ EXPORT TYPE=SOUND PACKAGE=\"%s\" NAME=\"%s\" FILE=\"%s\""),
							*Package, *Name, appFromAnsi( File ) );
						GEditor->Exec( l_chCmd );

						FString S = appFromAnsi( File );
						GLastDir[eLASTDIR_WAV] = S.Left( S.InStr( TEXT("\\"), 1 ) );
					}

					GFileManager->SetDefaultDirectory(appBaseDir());
				}
				break;

			case IDMN_MRU1:
			case IDMN_MRU2:
			case IDMN_MRU3:
			case IDMN_MRU4:
			case IDMN_MRU5:
			case IDMN_MRU6:
			case IDMN_MRU7:
			case IDMN_MRU8:
			{
				FString Filename = mrulist->Items[Command - IDMN_MRU1];
				GEditor->Exec( *(FString::Printf(TEXT("OBJ LOAD FILE=\"%s\""), *Filename )) );

				FString Package = Filename.Right( Filename.Len() - (Filename.InStr( TEXT("\\"), 1) + 1) );
				Package = Package.Left( Package.InStr( TEXT(".")) );

				GBrowserMaster->RefreshAll();
				pComboPackage->SetCurrent( pComboPackage->FindStringExact( *Package ) );
				RefreshGroups();
				RefreshSoundList();
			}
			break;

			case IDMN_SB_IMPORT_WAV:
				{
					OPENFILENAMEA ofn;
					char File[81920] = "\0";

					ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
					ofn.lStructSize = sizeof(OPENFILENAMEA);
					ofn.hwndOwner = hWnd;
					ofn.lpstrFile = File;
					ofn.nMaxFile = sizeof(char) * 81920;
					ofn.lpstrFilter = "Audio Files (*.wav,*.ogg)\0*.wav;*.ogg\0All Files\0*.*\0\0";
					ofn.lpstrDefExt = "wav";
					ofn.lpstrTitle = "Import Sounds";
					ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_WAV]) );
					ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_ALLOWMULTISELECT | OFN_EXPLORER;

					// Display the Open dialog box. 
					//
					if( GetOpenFileNameA(&ofn) )
					{
						int iNULLs = FormatFilenames( File );
						FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );
						FString Group = pComboGroup->GetString( pComboGroup->GetCurrent() );

						TArray<TCHAR> strFile;
		
						TArray<FString> StringArray;
						ParseStringToArray( TEXT("|"), appFromAnsi(strFile, File ), &StringArray );

						int iStart = 0;
						FString Prefix = TEXT("\0");

						if( iNULLs )
						{
							iStart = 1;
							Prefix = *(StringArray(0));
							Prefix += TEXT("\\");
						}

						if( StringArray.Num() == 1 )
							GLastDir[eLASTDIR_WAV] = StringArray(0).Left( StringArray(0).InStr( TEXT("\\"), 1 ) );
						else
							GLastDir[eLASTDIR_WAV] = StringArray(0);

						TArray<FString> FilenamesArray;

						for( int x = iStart ; x < StringArray.Num() ; x++ )
						{
							FString NewString;

							NewString = FString::Printf( TEXT("%s%s"), *Prefix, *(StringArray(x)) );
							new(FilenamesArray)FString( NewString );
						
							FString S = NewString;
						}

						WDlgImportSound l_dlg( NULL, this );
						l_dlg.DoModal( Package, Group, &FilenamesArray );

						GBrowserMaster->RefreshAll();
						pComboPackage->SetCurrent( pComboPackage->FindStringExact( *l_dlg.Package) );
						RefreshGroups();
						pComboGroup->SetCurrent( pComboGroup->FindStringExact( *l_dlg.Group) );
						RefreshSoundList();
					}

					GFileManager->SetDefaultDirectory(appBaseDir());
				}
				break;

			case IDMN_SB_PLAY:
				OnPlay();
				break;

			case IDMN_SB_STOP:
				OnStop();
				break;

			case IDMN_SB_FileSave:
				{
					OPENFILENAMEA ofn;
					char File[8192] = "\0";
					FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );

					::sprintf( File, "%s.uax", TCHAR_TO_ANSI( *Package ) );

					ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
					ofn.lStructSize = sizeof(OPENFILENAMEA);
					ofn.hwndOwner = hWnd;
					ofn.lpstrFile = File;
					ofn.nMaxFile = sizeof(char) * 8192;
					ofn.lpstrFilter = "Sound Packages (*.uax)\0*.uax\0All Files\0*.*\0\0";
					ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UAX]) );
					ofn.lpstrDefExt = "uax";
					ofn.lpstrTitle = "Save Sound Package";
					ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

					if( GetSaveFileNameA(&ofn) )
					{
						TCHAR l_chCmd[256];

						appSprintf( l_chCmd, TEXT("OBJ SAVEPACKAGE PACKAGE=\"%s\" FILE=\"%s\""),
							*Package, appFromAnsi( File ) );
						GEditor->Exec( l_chCmd );

						FString S = appFromAnsi( File );
						mrulist->AddItem( S );
						if( GBrowserMaster->GetCurrent()==BrowserID )
							mrulist->AddToMenu( hWnd, GetMenu( IsDocked() ? OwnerWindow->hWnd : hWnd ) );
						GLastDir[eLASTDIR_UAX] = S.Left( S.InStr( TEXT("\\"), 1 ) );
					}

					GFileManager->SetDefaultDirectory(appBaseDir());
				}
				break;

			case IDMN_SB_FileOpen:
				{
					OPENFILENAMEA ofn;
					char File[8192] = "\0";

					ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
					ofn.lStructSize = sizeof(OPENFILENAMEA);
					ofn.hwndOwner = hWnd;
					ofn.lpstrFile = File;
					ofn.nMaxFile = sizeof(char) * 8192;
					ofn.lpstrFilter = "Sound Packages (*.uax)\0*.uax\0All Files\0*.*\0\0";
					ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UAX]) );
					ofn.lpstrDefExt = "uax";
					ofn.lpstrTitle = "Open Sound Package";
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

						if( StringArray.Num() > 0 )
						{
							if( StringArray.Num() == 1 )
							{
								SavePkgName = *(StringArray(0));
								SavePkgName = SavePkgName.Right( SavePkgName.Len() - (SavePkgName.Left( SavePkgName.InStr(TEXT("\\"), 1)).Len() + 1 ));
							}
							else
								SavePkgName = *(StringArray(1));
							SavePkgName = SavePkgName.Left( SavePkgName.InStr(TEXT(".")) );
						}

						if( StringArray.Num() == 1 )
							GLastDir[eLASTDIR_UAX] = StringArray(0).Left( StringArray(0).InStr( TEXT("\\"), 1 ) );
						else
							GLastDir[eLASTDIR_UAX] = StringArray(0);

						GWarn->BeginSlowTask( TEXT(""), 1, 0 );

						for( int x = iStart ; x < StringArray.Num() ; x++ )
						{
							GWarn->StatusUpdatef( x, StringArray.Num(), TEXT("Loading %s"), *(StringArray(x)) );

							TCHAR l_chCmd[512];
							appSprintf( l_chCmd, TEXT("OBJ LOAD FILE=\"%s%s\""), *Prefix, *(StringArray(x)) );
							GEditor->Exec( l_chCmd );

							mrulist->AddItem( *(StringArray(x)) );
							if( GBrowserMaster->GetCurrent()==BrowserID )
								mrulist->AddToMenu( hWnd, GetMenu( IsDocked() ? OwnerWindow->hWnd : hWnd ) );
						}

						GWarn->EndSlowTask();

						GBrowserMaster->RefreshAll();
						pComboPackage->SetCurrent( pComboPackage->FindStringExact( *SavePkgName ) );
						RefreshGroups();
						RefreshSoundList();
					}

					GFileManager->SetDefaultDirectory(appBaseDir());
				}
				break;

			default:
				WBrowser::OnCommand(Command);
				break;
		}
		unguard;
	}
	virtual void RefreshAll()
	{
		guard(WBrowserSound::RefreshAll);
		RefreshPackages();
		RefreshGroups();
		RefreshSoundList();
		if( GBrowserMaster->GetCurrent()==BrowserID )
			mrulist->AddToMenu( hWnd, GetMenu( IsDocked() ? OwnerWindow->hWnd : hWnd ) );
		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WBrowserSound::OnSize);
		WBrowser::OnSize(Flags, NewX, NewY);
		PositionChildControls();
		InvalidateRect( hWnd, NULL, FALSE );
		UpdateMenu();
		unguard;
	}
	
	void RefreshPackages( void )
	{
		guard(WBrowserSound::RefreshPackages);

		// PACKAGES
		//
		pComboPackage->Empty();

		FStringOutputDevice GetPropResult = FStringOutputDevice();
		GEditor->Get( TEXT("OBJ"), TEXT("PACKAGES CLASS=Sound"), GetPropResult );

		TArray<FString> StringArray;
		ParseStringToArray( TEXT(","), *GetPropResult, &StringArray );

		for( int x = 0 ; x < StringArray.Num() ; x++ )
		{
			pComboPackage->AddString( *(StringArray(x)) );
		}

		pComboPackage->SetCurrent( 0 );
		unguard;
	}
	void RefreshGroups( void )
	{
		guard(WBrowserSound::RefreshGroups);

		FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );

		// GROUPS
		//
		pComboGroup->Empty();

		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];
		appSprintf( l_ch, TEXT("GROUPS CLASS=Sound PACKAGE=\"%s\""), *Package );
		GEditor->Get( TEXT("OBJ"), l_ch, GetPropResult );

		TArray<FString> StringArray;
		ParseStringToArray( TEXT(","), *GetPropResult, &StringArray );

		for( int x = 0 ; x < StringArray.Num() ; x++ )
		{
			pComboGroup->AddString( *(StringArray(x)) );
		}

		pComboGroup->SetCurrent( 0 );

		unguard;
	}
	void RefreshSoundList( void )
	{
		guard(WBrowserSound::RefreshSoundList);

		FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );
		FString Group = pComboGroup->GetString( pComboGroup->GetCurrent() );

		// SOUNDS
		//
		pListSounds->Empty();

		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		if( pCheckGroupAll->IsChecked() )
			appSprintf( l_ch, TEXT("QUERY TYPE=Sound PACKAGE=\"%s\""), *Package );
		else
 			appSprintf( l_ch, TEXT("QUERY TYPE=Sound PACKAGE=\"%s\" GROUP=\"%s\""), *Package, *Group );

		GEditor->Get( TEXT("OBJ"), l_ch, GetPropResult );

		TArray<FString> StringArray;
		ParseStringToArray( TEXT(" "), *GetPropResult, &StringArray );

		for( int x = 0 ; x < StringArray.Num() ; x++ )
		{
			pListSounds->AddString( *(StringArray(x)) );
		}

		pListSounds->SetCurrent( 0, 1 );

		UpdateCurrentSoundDetails ();

		unguard;
	}
	// Moves the child windows around so that they best match the window size.
	//
	void PositionChildControls( void )
	{
		guard(WBrowserSound::PositionChildControls);

		if( !pComboPackage 
			|| !pComboGroup
			|| !pListSounds
			|| !pCheckGroupAll ) return;

		FRect CR = GetClientRect();
		RECT R;
		::GetClientRect( hWndToolBar, &R );
		::MoveWindow( GetDlgItem( hWnd, ID_BS_TOOLBAR ), 0, 0, 2000, R.bottom, TRUE );

		float Top = (R.bottom - R.top) + 8;

		::MoveWindow( pComboPackage->hWnd, 4, Top, CR.Width() - 8, 20, 1 );
		Top += 22;
		::MoveWindow( pCheckGroupAll->hWnd, 4, Top, 32, 20, 1 );
		::MoveWindow( pComboGroup->hWnd, 4 + 32, Top, CR.Width() - 8 - 32, 20, 1 );
		Top += 26;

		::MoveWindow( pListSounds->hWnd, 4, Top, CR.Width() - 8, CR.Height() - Top - 4 - 22, 1 );
		Top = CR.Height() - 22;

		::MoveWindow( pSoundText->hWnd, 4, Top, 85, 20, 1);
		::MoveWindow( pFileType->hWnd, 90, Top, 40, 20, 1);
#if 0
		::MoveWindow( pCheckStreaming->hWnd, 140, Top, 60, 20, 1);
#else
//		::MoveWindow( pCheckLooping->hWnd, 140, Top, 60, 20, 1);
//		::MoveWindow( pCheckStreaming->hWnd, 210, Top, 70, 20, 1);
#endif

		::MoveWindow( pNumChannels->hWnd, 290, Top, 40, 20, 1);
		::MoveWindow( pLength->hWnd, 340, Top, 80, 20, 1);

		unguard;
	}

	// Notification delegates for child controls.
	//
	void OnComboPackageSelChange()
	{
		guard(WBrowserSound::OnComboPackageSelChange);
		RefreshGroups();
		RefreshSoundList();
		unguard;
	}
	void OnComboGroupSelChange()
	{
		guard(WBrowserSound::OnComboGroupSelChange);
		RefreshSoundList();
		unguard;
	}
	void OnListSoundsDblClick()
	{
		guard(WBrowserSound::OnListSoundsDblClick);
		OnPlay();
		unguard;
	}

	void UpdateCurrentSoundDetails ()
	{
		FString strFileType;
		GetCurrentFileType(strFileType);
		pFileType->SetText ( *strFileType );

//		bool bIsOgg = (appStricmp( *strFileType, TEXT("OGG")) == 0);

//		EnableWindow(pCheckLooping->hWnd, bIsOgg);
//		EnableWindow(pCheckStreaming->hWnd, bIsOgg);

//		pCheckLooping->SetCheck( IsCurrentFileLooping() );
//		pCheckStreaming->SetCheck (IsCurrentFileStreaming() );

//		pNumChannels->SetText ( IsCurrentFileStereo () ? TEXT("Stereo") : TEXT("Mono"));

//		GetCurrentFileULength (strFileType);
//		pLength->SetText ( *strFileType );

	}

	void OnPlay()
	{
		guard(WBrowserSound::OnPlay);

		TCHAR l_chCmd[256];

		FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );
		FString Group = pComboGroup->GetString( pComboGroup->GetCurrent () );
		FString Name = pListSounds->GetString( pListSounds->GetCurrent() );

		if (Group)
			appSprintf( l_chCmd, TEXT("AUDIO PLAY NAME=\"%s.%s.%s\""), *Package, *Group, *Name );
		else
			appSprintf( l_chCmd, TEXT("AUDIO PLAY NAME=\"%s.%s\""), *Package, *Name );

		GEditor->Exec( l_chCmd );
		unguard;
	}
	void OnStop()
	{
		guard(WBrowserSound::OnStop);
		GEditor->Exec( TEXT("AUDIO PLAY NAME=None") );
		unguard;
	}
	void OnGroupAllClick()
	{
		guard(WBrowserSound::OnGroupAllClick);
		EnableWindow( pComboGroup->hWnd, !pCheckGroupAll->IsChecked() );
		RefreshSoundList();
		unguard;
	}

	void GetCurrentFileType (FString & strFileType)
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
//		FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );
		FString Name = pListSounds->GetString( pListSounds->GetCurrent() );
		TCHAR l_ch[256];

//		appSprintf( l_ch, TEXT("FILETYPE PACKAGE=\"%s\" NAME=\"%s\""), *Package, *Name );
		appSprintf( l_ch, TEXT("FILETYPE NAME=\"%s\""), *Name );
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );

		strFileType = GetPropResult;
	}

	void GetCurrentFileULength (FString & strFileType )
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		appSprintf( l_ch, TEXT("INFO PATHNAME=\"%s\" MEMBER=ULength"), *GetCurrentPathName() );
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );

		strFileType = GetPropResult;
	}

#if 0
	bool IsCurrentFileLooping ()
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		appSprintf( l_ch, TEXT("INFO PATHNAME=\"%s\" MEMBER=Looping"), *GetCurrentPathName() );
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );

		return (appStricmp ( *GetPropResult, TEXT("0")) != 0);
	}

	bool IsCurrentFileStreaming ()
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		appSprintf( l_ch, TEXT("INFO PATHNAME=\"%s\" MEMBER=Streaming"), *GetCurrentPathName() );
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );

		return (appStricmp ( *GetPropResult, TEXT("0")) != 0);
	}
#endif
	
	bool IsCurrentFileStereo ()
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		appSprintf( l_ch, TEXT("INFO PATHNAME=\"%s\" MEMBER=Stereo"), *GetCurrentPathName() );
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );

		return (appStricmp ( *GetPropResult, TEXT("0")) != 0);
	}
	
#if 0
	void OnLoopingClick ()
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		appSprintf( l_ch, TEXT("SET PATHNAME=\"%s\" MEMBER=Looping VALUE=%i"), *GetCurrentPathName() , pCheckLooping->IsChecked ());
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );
	}

	void OnStreamingClick ()
	{
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		TCHAR l_ch[256];

		appSprintf( l_ch, TEXT("SET PATHNAME=\"%s\" MEMBER=Streaming VALUE=%i"), *GetCurrentPathName() , pCheckStreaming->IsChecked ());
		GEditor->Get( TEXT("Sound"), l_ch, GetPropResult );
	}
#endif

	virtual FString GetCurrentPathName( void )
	{
		guard(WBrowserSound::GetCurrentPathName);

		FString Package = pComboPackage->GetString( pComboPackage->GetCurrent() );
		FString Group = pComboGroup->GetString( pComboGroup->GetCurrent() );
		FString Name = pListSounds->GetString( pListSounds->GetCurrent() );

		if( Group.Len() )
			return *(FString::Printf(TEXT("%s.%s.%s"), *Package, *Group, *Name ));
		else
			return *(FString::Printf(TEXT("%s.%s"), *Package, *Name ));

		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
