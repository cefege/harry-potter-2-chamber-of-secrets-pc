/*=============================================================================
	BrowserGroup : Browser window for actor groups
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

    Work-in-progress todo's:

=============================================================================*/

#include <stdio.h>

extern void ParseStringToArray( const TCHAR* pchDelim, FString String, TArray<FString>* _pArray);
extern HWND GhwndEditorFrame;

// --------------------------------------------------------------
//
// NEW/RENAME GROUP Dialog
//
// --------------------------------------------------------------

class WDlgGroup : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgGroup,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WEdit NameEdit;

	FString defName, Name;
	UBOOL bNew;

	// Constructor.
	WDlgGroup( UObject* InContext, WBrowser* InOwnerWindow )
	:	WDialog			( TEXT("Group"), IDDIALOG_GROUP, InOwnerWindow )
	,	OkButton		( this, IDOK,			FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton	( this, IDCANCEL,		FDelegate(this,(TDelegate)EndDialogFalse) )
	,	NameEdit		( this, IDEC_NAME )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgGroup::OnInitDialog);
		WDialog::OnInitDialog();

		NameEdit.SetText( *defName );
		::SetFocus( NameEdit.hWnd );

		if( bNew )
			SetText(TEXT("New Group"));
		else
			SetText(TEXT("Rename Group"));

		NameEdit.SetSelection(0, -1);

		unguard;
	}
	virtual int DoModal( UBOOL InbNew, FString _defName )
	{
		guard(WDlgGroup::DoModal);

		bNew = InbNew;
		defName = _defName;

		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgGroup::OnOk);
		Name = NameEdit.GetText();
		EndDialog(TRUE);
		unguard;
	}
};

// --------------------------------------------------------------
//
// WBrowserGroup
//
// --------------------------------------------------------------

#define ID_BG_TOOLBAR	29050
TBBUTTON tbBGButtons[] = {
	{ 0, IDMN_MB_DOCK, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 1, IDMN_GB_NEW_GROUP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 2, IDMN_GB_DELETE_GROUP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 3, IDMN_GB_ADD_TO_GROUP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 4, IDMN_GB_DELETE_FROM_GROUP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 5, IDMN_GB_REFRESH, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 6, IDMN_GB_SELECT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 7, IDMN_GB_DESELECT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
};
struct {
	TCHAR ToolTip[64];
	int ID;
} ToolTips_BG[] = {
	TEXT("Toggle Dock Status"), IDMN_MB_DOCK,
	TEXT("New Group"), IDMN_GB_NEW_GROUP,
	TEXT("Delete"), IDMN_GB_DELETE_GROUP,
	TEXT("Add Selected Actors to Group(s)"), IDMN_GB_ADD_TO_GROUP,
	TEXT("Delete Select Actors from Group(s)"), IDMN_GB_DELETE_FROM_GROUP,
	TEXT("Refresh Group List"), IDMN_GB_REFRESH,
	TEXT("Select Actors in Group(s)"), IDMN_GB_SELECT,
	TEXT("Deselect Actors in Group(s)"), IDMN_GB_DESELECT,
	NULL, 0
};

class WBrowserGroup : public WBrowser
{
	DECLARE_WINDOWCLASS(WBrowserGroup,WBrowser,Window)

	WCheckListBox *pListGroups;
	HWND hWndToolBar;
	WToolTip *ToolTipCtrl;

	// Structors.
	WBrowserGroup( FName InPersistentName, WWindow* InOwnerWindow, HWND InEditorFrame )
	:	WBrowser( InPersistentName, InOwnerWindow, InEditorFrame )
	{
		pListGroups = NULL;
		MenuID = IDMENU_BrowserGroup;
		BrowserID = eBROWSER_GROUP;
		Description = TEXT("Groups");
	}

	// WBrowser interface.
	void OpenWindow( UBOOL bChild )
	{
		guard(WBrowserGroup::OpenWindow);
		WBrowser::OpenWindow( bChild );
		SetCaption();
		unguard;
	}
	virtual void UpdateMenu()
	{
		guard(WBrowserGroup::UpdateMenu);
		HMENU menu = IsDocked() ? GetMenu( OwnerWindow->hWnd ) : GetMenu( hWnd );
		CheckMenuItem( menu, IDMN_MB_DOCK, MF_BYCOMMAND | (IsDocked() ? MF_CHECKED : MF_UNCHECKED) );
		unguard;
	}
	void OnCreate()
	{
		guard(WBrowserGroup::OnCreate);
		WBrowser::OnCreate();

		SetMenu( hWnd, LoadMenuIdX(hInstance, IDMENU_BrowserGroup) );
		
		// GROUP LIST
		//
		pListGroups = new WCheckListBox( this, IDLB_GROUPS );
		pListGroups->OpenWindow( 1, 0, 1, 1 );

		hWndToolBar = CreateToolbarEx( 
			hWnd, WS_CHILD | WS_BORDER | WS_VISIBLE | CCS_ADJUSTABLE,
			IDB_BrowserGroup_TOOLBAR,
			8,
			hInstance,
			IDB_BrowserGroup_TOOLBAR,
			(LPCTBBUTTON)&tbBGButtons,
			11,
			16,16,
			16,16,
			sizeof(TBBUTTON));
		check(hWndToolBar);

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();
		for( int tooltip = 0 ; ToolTips_BG[tooltip].ID > 0 ; tooltip++ )
		{
			// Figure out the rectangle for the toolbar button.
			int index = SendMessageX( hWndToolBar, TB_COMMANDTOINDEX, ToolTips_BG[tooltip].ID, 0 );
			RECT rect;
			SendMessageX( hWndToolBar, TB_GETITEMRECT, index, (LPARAM)&rect);

			ToolTipCtrl->AddTool( hWndToolBar, ToolTips_BG[tooltip].ToolTip, tooltip, &rect );
		}

		RefreshGroupList();
		PositionChildControls();

		unguard;
	}
	virtual void RefreshAll()
	{
		guard(WBrowserGroup::RefreshAll);
		RefreshGroupList();
		unguard;
	}
	void OnDestroy()
	{
		guard(WBrowserGroup::OnDestroy);

		delete pListGroups;

		::DestroyWindow( hWndToolBar );
		delete ToolTipCtrl;

		WBrowser::OnDestroy();
		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WBrowserGroup::OnSize);
		WBrowser::OnSize(Flags, NewX, NewY);
		PositionChildControls();
		InvalidateRect( hWnd, NULL, FALSE );
		UpdateMenu();
		unguard;
	}
	// Updates the check status of all the groups, based on the contents of the VisibleGroups
	// variable in the LevelInfo.
	void GetFromVisibleGroups()
	{
		guard(WBrowserGroup::GetFromVisibleGroups);

		// First set all groups to "off"
		for( int x = 0 ; x < pListGroups->GetCount() ; x++ )
			pListGroups->SetItemData( x, 0 );

		// Now turn "on" the ones we need to.
		TArray<FString> Array;
		ParseStringToArray( TEXT(","), GEditor->Level->GetLevelInfo()->VisibleGroups, &Array );

		for( x = 0 ; x < Array.Num() ; x++ )
		{
			int Index = pListGroups->FindStringExact( *Array(x) );
			if( Index != LB_ERR )
				pListGroups->SetItemData( Index, 1 );
		}

		UpdateVisibility();
		unguard;
	}
	// Updates the VisibleGroups variable in the LevelInfp
	void SendToVisibleGroups()
	{
		guard(WBrowserGroup::SendToVisibleGroups);

		FString NewVisibleGroups;

		for( int x = 0 ; x < pListGroups->GetCount() ; x++ )
		{
			if( (int)pListGroups->GetItemData( x ) )
			{
				if( NewVisibleGroups.Len() )
					NewVisibleGroups += TEXT(",");
				NewVisibleGroups += pListGroups->GetString(x);
			}
		}

		GEditor->Level->GetLevelInfo()->VisibleGroups = NewVisibleGroups;

		GEditor->NoteSelectionChange( GEditor->Level );
		unguard;
	}
	void RefreshGroupList()
	{
		guard(WBrowserGroup::RefreshGroupList);

		// Loop through all the actors in the world and put together a list of unique group names.
		// Actors can belong to multiple groups by seperating the group names with semi-colons ("group1;group2")
		TArray<FString> Groups;

		for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
		{
			AActor* pActor = GEditor->Level->Actors(i);
			if(	pActor 
				&& !Cast<ACamera>(pActor )
				&& pActor!=GEditor->Level->Brush()
				&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
			{
				TArray<FString> Array;
				ParseStringToArray( TEXT(","), *pActor->Group, &Array );

				for( int x = 0 ; x < Array.Num() ; x++ )
				{
					// Only add the group name if it doesn't already exist.
					UBOOL bExists = FALSE;
					for( int z = 0 ; z < Groups.Num() ; z++ )
						if( Groups(z) == Array(x) )
						{
							bExists = 1;
							break;
						}

					if( !bExists )
						new(Groups)FString( Array(x) );
				}
			}
		}

		// Add the list of unique group names to the group listbox
		pListGroups->Empty();

		for( int x = 0 ; x < Groups.Num() ; x++ )
		{
			pListGroups->AddString( *Groups(x) );
			pListGroups->SetItemData( pListGroups->FindStringExact( *Groups(x) ), 1 );
		}

		GetFromVisibleGroups();

		pListGroups->SetCurrent( 0, 1 );
		unguard;
	}
	// Moves the child windows around so that they best match the window size.
	//
	void PositionChildControls( void )
	{
		guard(WBrowserGroup::PositionChildControls);

		if( !pListGroups ) return;

		FRect CR;
		CR = GetClientRect();
		RECT R;
		::GetClientRect( hWndToolBar, &R );

		::MoveWindow( pListGroups->hWnd, 4, R.bottom + 4, CR.Width() - 8, CR.Height() - 4 - R.bottom, 1 );

		unguard;
	}
	// Loops through all actors in the world and updates their visibility based on which groups are selected.
	void UpdateVisibility()
	{
		guard(WBrowserGroup::UpdateVisibility);

		// For each actor ...
		//
		// - break out its group field into seperate group names
		// - compare that list against the listbox - if any of those groups names are
		//   turned off, the actor is hidden.
		//
		FString NewVisibleGroups;

		for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
		{
			AActor* pActor = GEditor->Level->Actors(i);

			if(	pActor 
				&& !Cast<ACamera>(pActor )
				&& pActor!=GEditor->Level->Brush()
				&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
			{
				pActor->Modify();
				pActor->bHiddenEd = 0;

				TArray<FString> Array;
				ParseStringToArray( TEXT(","), *pActor->Group, &Array );

				for( int x = 0 ; x < Array.Num() ; x++ )
				{
					int Index = pListGroups->FindStringExact( *Array(x) );
					if( Index != LB_ERR && !(int)pListGroups->GetItemData( Index ) )
					{
						pActor->bHiddenEd = 1;
						break;
					}
				}
			}
		}

		PostMessageX( GhwndEditorFrame, WM_COMMAND, WM_REDRAWALLVIEWPORTS, 0 );
		unguard;
	}
	void OnCommand( INT Command )
	{
		guard(WBrowserGroup::OnCommand);
		switch( Command ) {

			case IDMN_GB_NEW_GROUP:
				NewGroup();
				break;

			case IDMN_GB_DELETE_GROUP:
				DeleteGroup();
				break;

			case IDMN_GB_ADD_TO_GROUP:
				{
					int SelCount = pListGroups->GetSelectedCount();
					if( SelCount == LB_ERR )	return;
					int* Buffer = new int[SelCount];
					pListGroups->GetSelectedItems(SelCount, Buffer);
					for( int s = 0 ; s < SelCount ; s++ )
						AddToGroup(pListGroups->GetString(Buffer[s]));
					delete [] Buffer;
				}
				break;

			case IDMN_GB_DELETE_FROM_GROUP:
				DeleteFromGroup();
				break;

			case IDMN_GB_RENAME_GROUP:
				RenameGroup();
				break;

			case IDMN_GB_REFRESH:
				OnRefreshGroups();
				break;

			case IDMN_GB_SELECT:
				SelectActorsInGroups(1);
				break;

			case IDMN_GB_DESELECT:
				SelectActorsInGroups(0);
				break;

			case WM_WCLB_UPDATE_VISIBILITY:
				UpdateVisibility();
				SendToVisibleGroups();
				break;

			default:
				WBrowser::OnCommand(Command);
				break;
		}
		unguard;
	}
	void SelectActorsInGroups( UBOOL Select )
	{
		guard(WBrowserGroup::SelectActorsInGroups);

		int SelCount = pListGroups->GetSelectedCount();
		if( SelCount == LB_ERR )	return;
		int* Buffer = new int[SelCount];
		pListGroups->GetSelectedItems(SelCount, Buffer);

		for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
		{
			AActor* pActor = GEditor->Level->Actors(i);
			if(	pActor 
				&& !Cast<ACamera>(pActor )
				&& pActor!=GEditor->Level->Brush()
				&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
			{
				FString GroupName = *pActor->Group;
				TArray<FString> Array;
				ParseStringToArray( TEXT(","), *GroupName, &Array );
				for( int x = 0 ; x < Array.Num() ; x++ )
				{
					int idx = pListGroups->FindStringExact( *Array(x) );

					for( int s = 0 ; s < SelCount ; s++ )
						if( idx == Buffer[s] )
							pActor->bSelected = Select;
				}
			}
		}

		delete [] Buffer;
		PostMessageX( GhwndEditorFrame, WM_COMMAND, WM_REDRAWALLVIEWPORTS, 0 );

		unguard;
	}
	void NewGroup()
	{
		guard(WBrowserGroup::NewGroup);

		if( !NumActorsSelected() )
		{
			appMsgf(TEXT("You must have some actors selected to create a new group."));
			return;
		}

		// Generate a suggested group name to use as a default.
		int x = 1;
		FString DefaultName;
		while(1)
		{
			DefaultName = *(FString::Printf(TEXT("Group%d"), x) );
			if( pListGroups->FindStringExact( *DefaultName ) == LB_ERR )
				break;
			x++;
		}

		WDlgGroup dlg( NULL, this );
		if( dlg.DoModal( 1, DefaultName ) )
		{
			if( GEditor->Level->GetLevelInfo()->VisibleGroups.Len() )
				GEditor->Level->GetLevelInfo()->VisibleGroups += TEXT(",");
			GEditor->Level->GetLevelInfo()->VisibleGroups += dlg.Name;

			AddToGroup( dlg.Name );
			RefreshGroupList();
		}

		unguard;
	}
	void DeleteGroup()
	{
		guard(WBrowserGroup::DeleteGroup);
		int SelCount = pListGroups->GetSelectedCount();
		if( SelCount == LB_ERR )	return;
		int* Buffer = new int[SelCount];
		pListGroups->GetSelectedItems(SelCount, Buffer);
		for( int s = 0 ; s < SelCount ; s++ )
		{
			FString DeletedGroup = pListGroups->GetString(Buffer[s]);
			for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
			{
				AActor* pActor = GEditor->Level->Actors(i);
				if(	pActor 
					&& !Cast<ACamera>(pActor )
					&& pActor!=GEditor->Level->Brush()
					&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
				{
					FString GroupName = *pActor->Group;
					TArray<FString> Array;
					ParseStringToArray( TEXT(","), *GroupName, &Array );
					FString NewGroup;
					for( int x = 0 ; x < Array.Num() ; x++ )
					{
						if( Array(x) != DeletedGroup )
						{
							if( NewGroup.Len() )
								NewGroup += TEXT(",");
							NewGroup += Array(x);
						}
					}
					if( NewGroup != *pActor->Group )
					{
						pActor->Modify();
						pActor->Group = *NewGroup;
					}
				}
			}
		}
		delete [] Buffer;

		GEditor->NoteSelectionChange( GEditor->Level );
		RefreshGroupList();
		unguard;
	}
	void AddToGroup( FString InGroupName)
	{
		guard(WBrowserGroup::AddToGroup);
		for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
		{
			AActor* pActor = GEditor->Level->Actors(i);
			if(	pActor 
				&& pActor->bSelected
				&& !Cast<ACamera>(pActor )
				&& pActor!=GEditor->Level->Brush()
				&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
			{
				FString GroupName = *pActor->Group, NewGroupName;

				// Make sure this actor is not already in this group.  If so, don't add it again.
				TArray<FString> Array;
				ParseStringToArray( TEXT(","), *GroupName, &Array );
				for( int x = 0 ; x < Array.Num() ; x++ )
				{
					if( Array(x) == InGroupName )
						break;
				}

				if( x == Array.Num() )
				{
					// Add the group to the actors group list
					NewGroupName = *(FString::Printf(TEXT("%s%s%s"), *GroupName, (GroupName.Len()?TEXT(","):TEXT("")), *InGroupName ) );
					
					// *** HP2 ( NameSize Check Added 7/23/02 ) ***
					// Check to make sure that the NewGroupName does not exceed the size limitation of an FName
					if( NewGroupName.Len() > NAME_SIZE-1 )
					{
						TCHAR buffer[1024];
						appSprintf(buffer, TEXT("Group Name for actor %s has exceded the limit of %d characters! \n\n If we added the newGroup %s it would be %d characters."), 
							pActor->GetName(), NAME_SIZE, *InGroupName, NewGroupName.Len() );
						
						// Let the user know that the group name is too large!
						::MessageBox( hWnd, buffer, TEXT("Setting Group failed!"), MB_OK );
						break;
					}
					// ********************************************

					pActor->Modify();
					pActor->Group = *NewGroupName;
				}
			}
		}

		GEditor->NoteSelectionChange( GEditor->Level );
		unguard;
	}
	void DeleteFromGroup()
	{
		guard(WBrowserGroup::DeleteFromGroup);
		int SelCount = pListGroups->GetSelectedCount();
		if( SelCount == LB_ERR )	return;
		int* Buffer = new int[SelCount];
		pListGroups->GetSelectedItems(SelCount, Buffer);
		for( int s = 0 ; s < SelCount ; s++ )
		{
			FString DeletedGroup = pListGroups->GetString(Buffer[s]);

			for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
			{
				AActor* pActor = GEditor->Level->Actors(i);
				if(	pActor 
					&& pActor->bSelected
					&& !Cast<ACamera>(pActor )
					&& pActor!=GEditor->Level->Brush()
					&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
				{
					FString GroupName = *pActor->Group;
					TArray<FString> Array;
					ParseStringToArray( TEXT(","), *GroupName, &Array );
					FString NewGroup;
					for( int x = 0 ; x < Array.Num() ; x++ )
						if( Array(x) != DeletedGroup )
						{
							if( NewGroup.Len() )
								NewGroup += TEXT(",");
							NewGroup += Array(x);
						}
					if( NewGroup != *pActor->Group )
					{
						pActor->Modify();
						pActor->Group = *NewGroup;
					}
				}
			}
		}

		GEditor->NoteSelectionChange( GEditor->Level );
		RefreshGroupList();
		unguard;
	}
	void RenameGroup()
	{
		guard(WBrowserGroup::RenameGroup);

		WDlgGroup dlg( NULL, this );
		int SelCount = pListGroups->GetSelectedCount();
		if( SelCount == LB_ERR )	return;
		int* Buffer = new int[SelCount];
		pListGroups->GetSelectedItems(SelCount, Buffer);
		for( int s = 0 ; s < SelCount ; s++ )
		{
			FString Src = pListGroups->GetString(Buffer[s]);
			if( dlg.DoModal( 0, Src ) )
				SwapGroupNames( Src, dlg.Name );
		}
		delete [] Buffer;

		RefreshGroupList();
		GEditor->NoteSelectionChange( GEditor->Level );

		unguard;
	}
	void SwapGroupNames( FString Src, FString Dst )
	{
		guard(WBrowserGroup::SwapGroupNames);

		if( Src == Dst ) return;
		check(Src.Len());
		check(Dst.Len());

		for( int i = 0 ; i < GEditor->Level->Actors.Num() ; i++ )
		{
			AActor* pActor = GEditor->Level->Actors(i);
			if(	pActor 
				&& !Cast<ACamera>(pActor )
				&& pActor!=GEditor->Level->Brush()
				&& pActor->GetClass()->GetDefaultActor()->bHiddenEd==0 )
			{
				FString GroupName = *pActor->Group, NewGroup;
				TArray<FString> Array;
				ParseStringToArray( TEXT(","), *GroupName, &Array );
				for( int x = 0 ; x < Array.Num() ; x++ )
				{
					FString AddName;
					AddName = Array(x);
					if( Array(x) == Src )
						AddName = Dst;

					if( NewGroup.Len() )
						NewGroup += TEXT(",");
					NewGroup += AddName;
				}

				if( NewGroup != *pActor->Group )
				{
					pActor->Modify();
					pActor->Group = *NewGroup;
				}
			}
		}
		unguard;
	}
	int NumActorsSelected()
	{
		guard(WBrowserGroup::NumActorsSelected);
		FStringOutputDevice GetPropResult = FStringOutputDevice();
		GEditor->Get( TEXT("ACTOR"), TEXT("NUMSELECTED"), GetPropResult );
		return appAtoi(*GetPropResult);
		unguard;
	}

	// Notification delegates for child controls.
	//
	void OnNewGroup()
	{
		guard(WBrowserGroup::OnNewGroupClick);
		NewGroup();
		unguard;
	}
	void OnDeleteGroup()
	{
		guard(WBrowserGroup::OnDeleteGroupClick);
		DeleteGroup();
		unguard;
	}
	void OnAddToGroup()
	{
		guard(WBrowserGroup::OnAddToGroupClick);
		int SelCount = pListGroups->GetSelectedCount();
		if( SelCount == LB_ERR )	return;
		int* Buffer = new int[SelCount];
		pListGroups->GetSelectedItems(SelCount, Buffer);
		for( int s = 0 ; s < SelCount ; s++ )
			AddToGroup(pListGroups->GetString(Buffer[s]));
		delete [] Buffer;
		unguard;
	}
	void OnDeleteFromGroup()
	{
		guard(WBrowserGroup::OnDeleteFromGroupClick);
		DeleteFromGroup();
		unguard;
	}
	void OnRefreshGroups()
	{
		guard(WBrowserGroup::OnRefreshGroupsClick);
		RefreshGroupList();
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
