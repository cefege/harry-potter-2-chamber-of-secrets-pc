/*=============================================================================
	BrowserMesh : Browser window for meshes
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include <stdio.h>

__declspec(dllimport) INT GLastScroll;

extern void Query( ULevel* Level, const TCHAR* Item, FString* pOutput );
extern void ParseStringToArray( const TCHAR* pchDelim, FString String, TArray<FString>* _pArray);

// --------------------------------------------------------------
//
// WBrowserMesh
//
// --------------------------------------------------------------

#define ID_MESH_TOOLBAR	29050
TBBUTTON tbMESHButtons[] = {
	{ 0, IDMN_MB_DOCK, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 1, IDPB_PLAY, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 2, IDPB_STOP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 3, IDPB_TWEN, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}

};
struct {
	TCHAR ToolTip[64];
	int ID;
} ToolTips_MESH[] = {
	TEXT("Toggle Dock Status"), IDMN_MB_DOCK,
	TEXT("Play Animation"), IDPB_PLAY,
	TEXT("Stop Animation"), IDPB_STOP,
	TEXT("Change Tween Rate"), IDPB_TWEN,

	NULL, 0
};

class WBrowserMesh : public WBrowser
{
	DECLARE_WINDOWCLASS(WBrowserMesh,WBrowser,Window)

	WComboBox* pMeshCombo;
	WListBox* pAnimList;
	HWND hWndToolBar;
	WToolTip *ToolTipCtrl;

	UViewport *pViewport;

	INT iFrame;
	float tweenTime;		//milestone 3

	// Structors.
	WBrowserMesh( FName InPersistentName, WWindow* InOwnerWindow, HWND InEditorFrame )
	:	WBrowser( InPersistentName, InOwnerWindow, InEditorFrame )
	{
		pMeshCombo = NULL;
		pAnimList = NULL;
		pViewport = NULL;
		iFrame = 0;
		BrowserID = eBROWSER_MESH;
		Description = TEXT("Meshes");
	}

	// WBrowser interface.
	void OpenWindow( UBOOL bChild )
	{
		guard(WBrowserMesh::OpenWindow);
		WBrowser::OpenWindow( bChild );
		SetCaption();
		Show(1);
		unguard;
	}
	void OnCreate()
	{
		guard(WBrowserMesh::OnCreate);
		WBrowser::OnCreate();

		pMeshCombo = new WComboBox( this, IDCB_MESH );
		pMeshCombo->OpenWindow( 1, 1 );

		pAnimList = new WListBox( this, IDLB_ANIMATIONS );
		pAnimList->OpenWindow( 1, 0, 0, 0, 0, LBS_MULTICOLUMN | WS_HSCROLL );
		SendMessageX( pAnimList->hWnd, LB_SETCOLUMNWIDTH, 196, 0 );
		pMeshCombo->SelectionChangeDelegate = FDelegate(this,(TDelegate)OnMeshSelectionChange);
		pAnimList->DoubleClickDelegate = FDelegate(this,(TDelegate)OnAnimDoubleClick);
		pAnimList->SelectionChangeDelegate = FDelegate(this,(TDelegate)OnAnimSelectionChange);
		tweenTime=0;

		hWndToolBar = CreateToolbarEx( 
			hWnd, WS_CHILD | WS_BORDER | WS_VISIBLE | CCS_ADJUSTABLE,
			IDB_BrowserMesh_TOOLBAR,
			4,
			hInstance,
			IDB_BrowserMesh_TOOLBAR,
			(LPCTBBUTTON)&tbMESHButtons,
			5,
			16,16,
			16,16,
			sizeof(TBBUTTON));
		check(hWndToolBar);

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();
		for( int tooltip = 0 ; ToolTips_MESH[tooltip].ID > 0 ; tooltip++ )
		{
			// Figure out the rectangle for the toolbar button.
			int index = SendMessageX( hWndToolBar, TB_COMMANDTOINDEX, ToolTips_MESH[tooltip].ID, 0 );
			RECT rect;
			SendMessageX( hWndToolBar, TB_GETITEMRECT, index, (LPARAM)&rect);

			ToolTipCtrl->AddTool( hWndToolBar, ToolTips_MESH[tooltip].ToolTip, tooltip, &rect );
		}

		RefreshAll();
		SetCaption();

		PositionChildControls();

		unguard;
	}
	void SetCaption( void )
	{
		guard(WBrowserMesh::SetCaption);

		FString Caption = TEXT("Mesh Browser");

		if( GetCurrentMeshName().Len() )
			Caption += FString::Printf( TEXT(" - %s"),
				GetCurrentMeshName() );

		SetText( *Caption );
		unguard;
	}
	virtual void RefreshAll()
	{
		guard(WBrowserMesh::RefreshAll);
		RefreshMeshList();
		RefreshAnimList();
		RefreshViewport();
		unguard;
	}
	void RefreshMeshList()
	{
		guard(WBrowserMesh::RefreshMeshList);

		FStringOutputDevice GetPropResult = FStringOutputDevice();
		GEditor->Get( TEXT("OBJ"), TEXT("Query Type=Mesh"), GetPropResult );

		pMeshCombo->Empty();

		TArray<FString> StringArray;
		ParseStringToArray( TEXT(" "), *GetPropResult, &StringArray );

		for( int x = 0 ; x < StringArray.Num() ; x++ )
			pMeshCombo->AddString( *(StringArray(x)) );

		pMeshCombo->SetCurrent(0);

		unguard;
	}
	FString GetCurrentMeshName()
	{
		guard(WBrowserMesh::GetCurrentMeshName);
		return pMeshCombo->GetString( pMeshCombo->GetCurrent() );
		unguard;
	}
	void RefreshAnimList()
	{
		guard(WBrowserMesh::RefreshAnimList);

		FString MeshName = GetCurrentMeshName();

		pAnimList->Empty();

		FStringOutputDevice GetPropResult = FStringOutputDevice();
		GEditor->Get( TEXT("MESH"), *(FString::Printf(TEXT("NUMANIMSEQS NAME=%s"), *MeshName)), GetPropResult );
		int NumAnims = appAtoi( *GetPropResult );

		for( int anim = 0 ; anim < NumAnims ; anim++ )
		{
			FStringOutputDevice GetPropResult = FStringOutputDevice();
			GEditor->Get( TEXT("MESH"), *(FString::Printf(TEXT("ANIMSEQ NAME=%s NUM=%d"), *MeshName, anim)), GetPropResult );

			int NumFrames = appAtoi( *(GetPropResult.Right(3)) );
			FString Name = GetPropResult.Left( GetPropResult.InStr(TEXT(" ")));

			pAnimList->AddString( *(FString::Printf(TEXT("%s [ %d ]"), *Name, NumFrames )) );
		}

		pAnimList->SetCurrent(0, 1);

		unguard;
	}
	void RefreshViewport()
	{
		guard(WBrowserMesh::RefreshViewport);

		if( !pViewport )
		{
			// Create the mesh viewport
			//
			pViewport = GEditor->Client->NewViewport( TEXT("MeshViewer") );
			check(pViewport);
			GEditor->Level->SpawnViewActor( pViewport );
			pViewport->Input->Init( pViewport );
			check(pViewport->Actor);
			pViewport->Actor->ShowFlags = SHOW_StandardView | SHOW_NoButtons | SHOW_ChildWindow;
			pViewport->Actor->RendMap   = REN_MeshView;
			pViewport->Group = NAME_None;
			pViewport->MiscRes = UObject::StaticFindObject( NULL, ANY_PACKAGE, *(pMeshCombo->GetString(pMeshCombo->GetCurrent())) );
			check(pViewport->MiscRes);
			pViewport->Actor->Misc1 = 0;
			pViewport->Actor->Misc2 = 0;

			pViewport->OpenWindow( (DWORD)hWnd, 0, 256, 256, 0, 0 );
		}
		else
		{
			FString MeshName = pMeshCombo->GetString(pMeshCombo->GetCurrent());

			FStringOutputDevice GetPropResult = FStringOutputDevice();
			GEditor->Get( TEXT("MESH"), *(FString::Printf(TEXT("ANIMSEQ NAME=\"%s\" NUM=%d"), *MeshName, pAnimList->GetCurrent())), GetPropResult );

			GEditor->Exec( *(FString::Printf(TEXT("CAMERA UPDATE NAME=MeshViewer MESH=\"%s\" FLAGS=%d REN=%d MISC1=%d MISC2=%d"),
				*MeshName,
				iFrame < 0
				? SHOW_StandardView | SHOW_NoButtons | SHOW_ChildWindow | SHOW_Backdrop | SHOW_RealTime
				: SHOW_StandardView | SHOW_NoButtons | SHOW_ChildWindow,
				REN_MeshView,
				appAtoi(*(GetPropResult.Right(7).Left(3))),
				iFrame
				)));
		}

		unguard;
	}
	void OnDestroy()
	{
		guard(WBrowserMesh::OnDestroy);

		GEditor->Exec( TEXT("CAMERA CLOSE NAME=MeshViewer") );
		delete pViewport;

		delete pMeshCombo;
		delete pAnimList;

		::DestroyWindow( hWndToolBar );
		delete ToolTipCtrl;

		WBrowser::OnDestroy();
		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WBrowserMesh::OnSize);
		WBrowser::OnSize(Flags, NewX, NewY);
		PositionChildControls();
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void PositionChildControls()
	{
		guard(WBrowserMesh::PositionChildControls);
		if( !pMeshCombo || !::IsWindow( pMeshCombo->hWnd )
				|| !pAnimList || !::IsWindow( pAnimList->hWnd )
				|| !pViewport
				)
			return;

		LockWindowUpdate( hWnd );

		FRect CR;
		CR = GetClientRect();
		RECT R;
		::GetClientRect( hWndToolBar, &R );

		float Top = CR.Min.Y + R.bottom + 12;
		::MoveWindow( pMeshCombo->hWnd, 4, Top, CR.Width() - 8, 20, 1 );
		Top += 20;
		::MoveWindow( pAnimList->hWnd, 4, Top, CR.Width() - 8, 96, 1 );
		Top += 96;
		::MoveWindow( (HWND)pViewport->GetWindow(), 4, Top, CR.Width() - 8, CR.Height() - Top, 1 );
		pViewport->Repaint( 1 );

		// Refresh the display.
		LockWindowUpdate( NULL );

		unguard;
	}
	void OnPlay()
	{
		guard(WBrowserMesh::OnPlay);
		iFrame = -1;
		RefreshViewport();
		unguard;
	}
	void OnStop()
	{
		guard(WBrowserMesh::OnStop);
		iFrame++;
		RefreshViewport();
		unguard;
	}
	void OnCommand( INT Command )
	{
		extern bool CALLBACK TweenDialogProc( HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam );

		guard(WBrowserMesh::OnCommand);
		switch( Command ) {

			case IDPB_PLAY:
				OnPlay();
				break;

			case IDPB_STOP:
				OnStop();
				break;

			case IDPB_TWEN:
				//HWND hwnd;

				DialogBoxParam(hInstance,MAKEINTRESOURCE(IDD_TWENRATE),hWndToolBar,(DLGPROC)TweenDialogProc,(LPARAM)&tweenTime);
			//	ShowWindow(hwnd,SW_SHOWNORMAL);
				break;
			default:
				WBrowser::OnCommand(Command);
				break;
		}
		unguard;
	}

	// Notification delegates for child controls.
	//
	void OnMeshSelectionChange()
	{
		guard(WBrowserMesh::OnMeshSelectionChange);
		RefreshAnimList();
		RefreshViewport();
		SetCaption();
		unguard;
	}
	void OnAnimDoubleClick()
	{
		guard(WBrowserMesh::OnAnimDoubleClick);
		OnPlay();
		unguard;
	}
	void OnAnimSelectionChange()
	{
		guard(WBrowserMesh::OnAnimSelectionChange);
		if(tweenTime!=0)		//milestone 3
		{
			pViewport->Actor->TweenRate=1.0/(tweenTime/3);		
			pViewport->Actor->TweenAlpha=0;
		}

		RefreshViewport();
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
