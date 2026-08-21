/*=============================================================================
	MatineeSheet : Property sheet for terrain editing
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include "..\..\editor\inc\UnMatinee.h"

class WViewportFrame;
class WLevelFrame;
extern WLevelFrame* GLevelFrame;

// --------------------------------------------------------------
//
// WPageMatGeneral
//
// --------------------------------------------------------------

struct {
	TCHAR ToolTip[64];
	INT ID;
} ToolTips_PageMatGeneral[] = {
	TEXT("Always display the camera path, regardless of selections?"), IDCK_ALWAYS_SHOW_PATH,
	TEXT("Display orientation markers on path?"), IDCK_SHOW_PATH_ORIENTATION,
	TEXT("Drag thumb to move camera along path"), IDSL_POSITION,
	TEXT("Begin automatic playback"), IDPB_EXECUTE,
	TEXT("Stop automatic playback"), IDPB_STOP,
	TEXT("Play backwards"), IDPB_BACKWARD,
	TEXT("Play Forwards"), IDPB_FORWARD,
	NULL, 0
};

class WPageMatGeneral : public WPropertyPage
{
	DECLARE_WINDOWCLASS(WPageMatGeneral,WPropertyPage,Window)

	WGroupBox *GeneralBox, *OptionsBox, *PlaybackBox;
	WTrackBar *PositionBar;
	WButton *BackwardButton, *ExecuteButton, *ForwardButton, *StopButton;
	HBITMAP BackwardBitmap, ExecuteBitmap, ForwardBitmap, StopBitmap;
	UViewport *ViewportIP;
	WScrollBar *ScrollBar;
	WComboBox *PosCombo;
	WCheckBox *AlwaysShowPathCheck, *ShowPathOrientationCheck;
	WViewportFrame *PreviewWindow;

	WToolTip* ToolTipCtrl;

	TArray<FPosition> PlaybackPositions;

	// Structors.
	WPageMatGeneral ( WWindow* InOwnerWindow )
	:	WPropertyPage( InOwnerWindow )
	{
		GeneralBox = OptionsBox = PlaybackBox = NULL;
		PositionBar = NULL;
		ViewportIP = NULL;
		ScrollBar = NULL;
		PosCombo = NULL;
		BackwardButton = ExecuteButton = ForwardButton = StopButton = NULL;
		AlwaysShowPathCheck = ShowPathOrientationCheck = NULL;
		GMatineeIPCamLocation = FVector(0,0,0);
	
		BackwardBitmap = (HBITMAP)LoadImageA( hInstance, MAKEINTRESOURCEA(IDBM_BACKWARD), IMAGE_BITMAP, 0, 0, LR_LOADMAP3DCOLORS );	check(BackwardBitmap);
		ExecuteBitmap = (HBITMAP)LoadImageA( hInstance, MAKEINTRESOURCEA(IDBM_EXECUTE), IMAGE_BITMAP, 0, 0, LR_LOADMAP3DCOLORS );	check(ExecuteBitmap);
		ForwardBitmap = (HBITMAP)LoadImageA( hInstance, MAKEINTRESOURCEA(IDBM_FORWARD), IMAGE_BITMAP, 0, 0, LR_LOADMAP3DCOLORS );	check(ForwardBitmap);
		StopBitmap = (HBITMAP)LoadImageA( hInstance, MAKEINTRESOURCEA(IDBM_STOP), IMAGE_BITMAP, 0, 0, LR_LOADMAP3DCOLORS );	check(StopBitmap);
	}

	virtual void OpenWindow( INT InDlgId, HMODULE InHMOD )
	{
		guard(WPageMatGeneral::OpenWindow);
		WPropertyPage::OpenWindow( InDlgId, InHMOD );

		// Create child controls and let the base class determine their proper positions.
		GeneralBox = new WGroupBox( this, IDGP_GENERAL );
		GeneralBox->OpenWindow( 1, 0 );
		OptionsBox = new WGroupBox( this, IDGP_OPTIONS );
		OptionsBox->OpenWindow( 1, 0 );
		PlaybackBox = new WGroupBox( this, IDGP_PLAYBACK );
		PlaybackBox->OpenWindow( 1, 0 );
		PositionBar = new WTrackBar( this, IDSL_POSITION );
		PositionBar->OpenWindow( 1);
		ScrollBar = new WScrollBar( this, IDSB_SCROLLBAR );
		ScrollBar->OpenWindow( 1, 0, 0, 10, 10, 0 );
		PosCombo = new WComboBox( this, IDCB_POS );
		PosCombo->OpenWindow( 1, 0 );
		AlwaysShowPathCheck = new WCheckBox( this, IDCK_ALWAYS_SHOW_PATH );
		AlwaysShowPathCheck->OpenWindow( 1, 0, 0, 10, 10, TEXT("") );
		ShowPathOrientationCheck = new WCheckBox( this, IDCK_SHOW_PATH_ORIENTATION );
		ShowPathOrientationCheck->OpenWindow( 1, 0, 0, 10, 10, TEXT("") );
		BackwardButton = new WButton( this, IDPB_BACKWARD, FDelegate(this,(TDelegate)OnBackward) );
		BackwardButton->OpenWindow( 1, 0, 0, 10, 10, TEXT("<") );
		ExecuteButton = new WButton( this, IDPB_EXECUTE, FDelegate(this,(TDelegate)OnExecute) );
		ExecuteButton->OpenWindow( 1, 0, 0, 10, 10, TEXT("!") );
		ForwardButton = new WButton( this, IDPB_FORWARD, FDelegate(this,(TDelegate)OnForward) );
		ForwardButton->OpenWindow( 1, 0, 0, 10, 10, TEXT(">") );
		StopButton = new WButton( this, IDPB_STOP, FDelegate(this,(TDelegate)OnStop) );
		StopButton ->OpenWindow( 1, 0, 0, 10, 10, TEXT(">") );

		PlaceControl( GeneralBox );
		PlaceControl( OptionsBox );
		PlaceControl( PlaybackBox );
		PlaceControl( PositionBar );
		PlaceControl( ScrollBar );
		PlaceControl( PosCombo );
		PlaceControl( AlwaysShowPathCheck );
		PlaceControl( ShowPathOrientationCheck );
		PlaceControl( BackwardButton );
		PlaceControl( ExecuteButton );
		PlaceControl( ForwardButton );
		PlaceControl( StopButton );

		Finalize();

		// Delegates.
		PosCombo->SelectionChangeDelegate = FDelegate(this,(TDelegate)OnPosSelChange);
		AlwaysShowPathCheck->ClickDelegate = FDelegate(this, (TDelegate)OnAlwaysShowPathClicked);
		ShowPathOrientationCheck->ClickDelegate = FDelegate(this, (TDelegate)OnShowPathOrientationClicked);
		PositionBar->ThumbTrackDelegate = FDelegate(this, (TDelegate)UpdateViewports);
		PositionBar->ThumbPositionDelegate = FDelegate(this, (TDelegate)UpdateViewports);
		PositionBar->SetTicFreq( 200 );

		AlwaysShowPathCheck->SetCheck( matAlwaysShowPath );
		ShowPathOrientationCheck->SetCheck( matShowPathOrientation );
		ForwardButton->SetCheck(1);

		BackwardButton->SetBitmap( BackwardBitmap );
		ExecuteButton->SetBitmap( ExecuteBitmap );
		ForwardButton->SetBitmap( ForwardBitmap );
		StopButton->SetBitmap( StopBitmap );

		// Create the interpolation point viewport
		FName Name = TEXT("MatineeIP");
		ViewportIP = GEditor->Client->NewViewport( Name );
		GEditor->Level->SpawnViewActor( ViewportIP );
		ViewportIP->Actor->ShowFlags = SHOW_StandardView | SHOW_ChildWindow;
		ViewportIP->Actor->RendMap   = REN_MatineeIP;
		ViewportIP->Actor->Misc1 = 0;
		ViewportIP->Actor->Misc2 = 0;
		ViewportIP->Group = NAME_None;
		ViewportIP->MiscRes = NULL;
		ViewportIP->Input->Init( ViewportIP );

		RECT rc;
		::GetWindowRect( GetDlgItem( hWnd, IDSC_VIEWPORTIP ), &rc );
		::ScreenToClient( hWnd, (POINT*)&rc.left );
		::ScreenToClient( hWnd, (POINT*)&rc.right );
		ViewportIP->OpenWindow( (DWORD)hWnd, 0, (rc.right - rc.left), (rc.bottom - rc.top), rc.left, rc.top );

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();
		for( INT tooltip = 0 ; ToolTips_PageMatGeneral[tooltip].ID > 0 ; tooltip++ )
			ToolTipCtrl->AddTool( GetDlgItem( hWnd, ToolTips_PageMatGeneral[tooltip].ID ), ToolTips_PageMatGeneral[tooltip].ToolTip, tooltip );

		unguard;
	}
	// Updates all appropriate viewports to the current position specified on the trackbar
	void UpdateViewports()
	{
		guard(WPageMatGeneral::UpdateViewports);

		INT idx = PositionBar->GetPos();
		FPosition* Pos = &PlaybackPositions(idx);
		if( !Pos ) return;

		UViewport* Viewport;
		for( INT vp = 0 ; vp < dED_MAX_VIEWPORTS ; vp++ )
		{
			Viewport = FindObject<UViewport>( ANY_PACKAGE, *(FString::Printf(TEXT("U2Viewport%d"), vp) ) );
			if( Viewport )
				if( !Viewport->IsOrtho() )
				{
					Viewport->Actor->Location = Pos->Location;
					Viewport->Actor->Rotation = Pos->Rotation;
					Viewport->Repaint(1);
				}
		}

		unguard;
	}
	void OnDestroy()
	{
		guard(WPageMatGeneral::OnDestroy);
		WPropertyPage::OnDestroy();

		::DestroyWindow( GeneralBox->hWnd );
		::DestroyWindow( OptionsBox->hWnd );
		::DestroyWindow( PlaybackBox->hWnd );
		::DestroyWindow( PositionBar->hWnd );
		::DestroyWindow( ScrollBar->hWnd );
		::DestroyWindow( PosCombo->hWnd );
		::DestroyWindow( AlwaysShowPathCheck->hWnd );
		::DestroyWindow( ShowPathOrientationCheck->hWnd );
		::DestroyWindow( BackwardButton->hWnd );
		::DestroyWindow( ExecuteButton->hWnd );
		::DestroyWindow( ForwardButton->hWnd );
		::DestroyWindow( StopButton->hWnd );

		delete GeneralBox;
		delete OptionsBox;
		delete PlaybackBox;
		delete PositionBar;
		delete ScrollBar;
		delete PosCombo;
		delete AlwaysShowPathCheck;
		delete ShowPathOrientationCheck;
		delete BackwardButton;
		delete ExecuteButton;
		delete ForwardButton;
		delete StopButton;

		DeleteObject( BackwardBitmap );
		DeleteObject( ExecuteBitmap );
		DeleteObject( ForwardBitmap );
		DeleteObject( StopBitmap );

		delete ViewportIP;

		KillTimer( hWnd, 5 );

		delete ToolTipCtrl;

		unguard;
	}
	virtual void Refresh()
	{
		guard(WPageMatGeneral::Refresh);
		WPropertyPage::Refresh();

		ViewportIP->Repaint(1);
		RefreshScrollBar();
		RefreshPosCombo();
		RefeshPlaybackPositions();

		unguard;
	}
	void RefreshPosCombo()
	{
		guard(WPageMatGeneral::RefreshPosCombo);
		PosCombo->Empty();

		for( INT x = 0 ; x < matGetCount() ; x++ )
			PosCombo->AddString( *FString::Printf(TEXT("%d"), matIPList[x]->Position ) );
		PosCombo->SetCurrent(0);

		unguard;
	}
	void RefreshViewport()
	{
		guard(WPageMatGeneral::RefreshViewport);
		ViewportIP->Repaint( 1 );
		unguard;
	}
	void RefreshScrollBar()
	{
		guard(WPageMatGeneral::RefreshScrollBar);
		if( !ScrollBar ) return;

		// Set the scroll bar to have a valid range.
		//
		SCROLLINFO si;
		si.cbSize = sizeof(SCROLLINFO);
		si.fMask = SIF_DISABLENOSCROLL | SIF_RANGE | SIF_POS;
		si.nMin = 0;
		si.nMax = (INT)matGetLength();
		si.nPos = (INT)GMatineeIPCamLocation.X;
		SetScrollInfo( ScrollBar->hWnd, SB_CTL, &si, TRUE );
		unguard;
	}
	virtual void OnHScroll( WPARAM wParam, LPARAM lParam )
	{
		if( (HWND)lParam == ScrollBar->hWnd )
		{
			switch(LOWORD(wParam)) {

				case SB_LINEUP:
					GMatineeIPCamLocation.X -= 4;
					GMatineeIPCamLocation.X = Max( GMatineeIPCamLocation.X, 0.f );
					RefreshViewport();
					break;

				case SB_LINEDOWN:
					GMatineeIPCamLocation.X += 4;
					GMatineeIPCamLocation.X = Min( GMatineeIPCamLocation.X, matGetLength() );
					RefreshViewport();
					break;

				case SB_PAGEUP:
					GMatineeIPCamLocation.X -= 16;
					GMatineeIPCamLocation.X = Max( GMatineeIPCamLocation.X, 0.f );
					RefreshViewport();
					break;

				case SB_PAGEDOWN:
					GMatineeIPCamLocation.X += 16;
					GMatineeIPCamLocation.X = Min( GMatineeIPCamLocation.X, matGetLength() );
					RefreshViewport();
					break;

				case SB_THUMBTRACK:
					GMatineeIPCamLocation.X = (short int)HIWORD(wParam);
					RefreshViewport();
					break;
			}

			RefreshScrollBar();
		}
	}
	void RefeshPlaybackPositions()
	{
		guard(WPageMatGeneral::RefeshPlaybackPositions);

		matGetPlaybackPositions( ViewportIP, &PlaybackPositions );

		PositionBar->SetRange( 0, PlaybackPositions.Num() );

		unguard;
	}
	void OnTimer()
	{
		guard(WPageMatGeneral::OnTimer);

		ViewportIP->Actor->GetLevel()->Tick( LEVELTICK_ViewportsOnly, .1f );
			
		// Advance the position bar
		INT idx = PositionBar->GetPos();

		if( ForwardButton->IsChecked() )
		{
			idx++;
			if( idx >= PlaybackPositions.Num() ) idx = 0;
		}
		else
		{
			idx--;
			if( idx < 0 ) idx = PlaybackPositions.Num() - 1;
		}

		PositionBar->SetPos(idx);
		
		UpdateViewports();
		
		unguard;
	}

	void OnPosSelChange()
	{
		guard(WPageMatGeneral::OnPosSelChange);
		//matSyncToPos( PosCombo->GetCurrent() );
		//RefreshViewport();
		//RefreshScrollBar();
		unguard;
	}
	void OnAlwaysShowPathClicked()
	{
		guard(WPageMatGeneral::OnAlwaysShowPathClicked);
		matAlwaysShowPath = AlwaysShowPathCheck->IsChecked();
		GEditor->RedrawLevel( ViewportIP->Actor->GetLevel() );
		unguard;
	}
	void OnShowPathOrientationClicked()
	{
		guard(WPageMatGeneral::OnShowPathOrientationClicked);
		matShowPathOrientation = ShowPathOrientationCheck->IsChecked();
		GEditor->RedrawLevel( ViewportIP->Actor->GetLevel() );
		unguard;
	}
	void OnBackward()
	{
	}
	void OnForward()
	{
	}
	void OnExecute()
	{
		SetTimer( hWnd, 5, 10, NULL );
		EnableWindow( ExecuteButton->hWnd, 0 );
		RefeshPlaybackPositions();
	}
	void OnStop()
	{
		KillTimer( hWnd, 5 );
		EnableWindow( ExecuteButton->hWnd, 1 );
	}
};

// --------------------------------------------------------------
//
// WMatineeSheet
//
// --------------------------------------------------------------

class WMatineeSheet : public WWindow
{
	DECLARE_WINDOWCLASS(WMatineeSheet,WWindow,Window)

	WPropertySheet* PropSheet;
	WPageMatGeneral* GeneralPage;
	LONG SaveActorPropertiesStyle;
	FRect SaveActorPropertiesRect;
	HWND SaveActorPropertiesParent;

	// Structors.
	WMatineeSheet( FName InPersistentName, WWindow* InOwnerWindow )
	:	WWindow( InPersistentName, InOwnerWindow )
	{
	}

	// WMatineeSheet interface.
	void OpenWindow()
	{
		guard(WMatineeSheet::OpenWindow);
		MdiChild = 0;
		PerformCreateWindowEx
		(
			NULL,
			TEXT("Matinee Controls"),
			WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
			0, 0,
			0, 0,
			OwnerWindow ? OwnerWindow->hWnd : NULL,
			NULL,
			hInstance
		);
 
		unguard;
	}
	void OnCreate()
	{
		guard(WMatineeSheet::OnCreate);
		WWindow::OnCreate();

		// Create the sheet
		PropSheet = new WPropertySheet( this, IDPS_MATINEE );
		PropSheet->OpenWindow( 1, 0 );

		// Create the pages for the sheet
		GeneralPage = new WPageMatGeneral( PropSheet->Tabs );
		GeneralPage->OpenWindow( IDPP_MAT_GENERAL, GetModuleHandleA("unrealed.exe") );
		PropSheet->AddPage( GeneralPage );

		PropSheet->SetCurrent( 0 );

		// Resize the property sheet to surround the pages properly.
		RECT rect;
		::GetClientRect( GeneralPage->hWnd, &rect );
		::SetWindowPos( hWnd, HWND_TOP, 0, 0, rect.right + 32 + 256, rect.bottom + 64, SWP_NOMOVE );

		PositionChildControls();

		// Remove the "X" button.
		LONG Style = GetWindowLongA( hWnd, GWL_STYLE );
		Style &= ~WS_SYSMENU;
		SetWindowLongA( hWnd, GWL_STYLE, Style );

		unguard;
	}
	virtual void OnShowWindow( UBOOL bShow )
	{
		guard(WMatineeSheet::OnShowWindow);
		WWindow::OnShowWindow( bShow );

		if( GEditor && GEditor->ActorProperties )
		{
			if( bShow )
			{
				// Customize the actor properties window.
				LONG Style = SaveActorPropertiesStyle = GetWindowLongA( GEditor->ActorProperties->hWnd, GWL_STYLE );
				Style &= ~WS_SYSMENU;
				Style &= ~WS_CAPTION;
				Style &= ~WS_THICKFRAME;
				SetWindowLongA( GEditor->ActorProperties->hWnd, GWL_STYLE, Style );
				SaveActorPropertiesRect = GEditor->ActorProperties->GetWindowRect();
				SaveActorPropertiesParent = GetParent( GEditor->ActorProperties->hWnd );
				SetParent( GEditor->ActorProperties->hWnd, PropSheet->hWnd );

			}
			else
			{
				// Restore the old state of the actor property window.
				SetWindowLongA( GEditor->ActorProperties->hWnd, GWL_STYLE, SaveActorPropertiesStyle );
				GEditor->ActorProperties->MoveWindow( SaveActorPropertiesRect, 1 );
				SetParent( GEditor->ActorProperties->hWnd, SaveActorPropertiesParent );
			}

			SetWindowPos( GEditor->ActorProperties->hWnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE );
		}

		unguard;
	}
	void OnDestroy()
	{
		guard(WMatineeSheet::OnDestroy);
		WWindow::OnDestroy();

		delete GeneralPage;
		delete PropSheet;

		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WMatineeSheet::OnSize);
		WWindow::OnSize(Flags, NewX, NewY);
		PositionChildControls();
		InvalidateRect( hWnd, NULL, FALSE );

		AlignPropertiesWindow();

		unguard;
	}
	void OnMove( INT NewX, INT NewY )
	{
		guard(WMatineeSheet::OnMove);
		WWindow::OnMove(NewX, NewY);

		AlignPropertiesWindow();

		unguard;
	}
	void Show( UBOOL Show )
	{
		guard(WMatineeSheet::Show);
		WWindow::Show( Show );
		if( Show) AlignPropertiesWindow();
		unguard;
	}
	void PositionChildControls()
	{
		guard(WMatineeSheet::PositionChildControls);
		if( !PropSheet || !::IsWindow( PropSheet->hWnd )
				)
			return;

		FRect CR = GetClientRect();
		::MoveWindow( PropSheet->hWnd, 0, 0, CR.Width(), CR.Height(), 1 );

		unguard;
	}
	void AlignPropertiesWindow()
	{
		guard(WMatineeSheet::AlignPropertiesWindow);

		if( !GEditor->ActorProperties ) return;

		FRect rect = GetClientRect();
		rect.Min.X = rect.Max.X - (256+16);
		rect.Max.X = rect.Min.X + 256;

		rect.Min.Y += 32;
		rect.Max.Y -= 12;

		GEditor->ActorProperties->MoveWindow( rect, 1 );

		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/