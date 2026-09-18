// PackageTool.cpp
// Copyright (c) 1999, Andrew Scheidecker


#include "PackageTool.h"


///////////////////////////////////////
// Globals
///////////////////////////////////////

IMPLEMENT_PACKAGE(PackageTool);

WPackageToolWindow*			MainWindow;					// The main window object.

FMallocWindows				Malloc;						// Memory allocation.
FOutputDeviceFile			Log;						// Log.
FOutputDeviceWindowsError	Error;						// Error.
FFeedbackContextWindows		Warn;						// Feedback.
FFileManagerWindows			FileManager;				// File manager.

WNDPROC						WTreeControl::SuperProc;


//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
// FTreeControl
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////


///////////////////////////////////////
// FTreeControlItem constructor
///////////////////////////////////////

FTreeControlItem::FTreeControlItem(WTreeControl* InTreeControl,UObject* InObject)
{
	guard(FTreeControlItem::FTreeControlItem);

	TreeControl = InTreeControl;
	Expanded = FoundChildren = 0;

	Object = InObject;

	// Construct the item name.

	ItemName += Object->GetClass()->GetName();
	ItemName += TEXT("\'");
	ItemName += Object->GetName();
	ItemName += TEXT("\'");

	unguard;
}

///////////////////////////////////////
// FTreeControlItem destructor
///////////////////////////////////////

FTreeControlItem::~FTreeControlItem()
{
	guard(FTreeControlItem::~FTreeControlItem);

	// Delete all child items.

	for(int Index = 0;Index < ChildItems.Num();Index++)
		delete ChildItems(Index);

	unguard;
}

///////////////////////////////////////
// FTreeControlItem::Created
///////////////////////////////////////

void FTreeControlItem::Created()
{
	guard(FTreeControlItem::Created);

	if(!ParentItem)
		InitChildren();

	unguard;
}

///////////////////////////////////////
// FTreeControlItem::Expand
///////////////////////////////////////

void FTreeControlItem::Expand()
{
	guard(FTreeControlItem::Expand);

	GLog->Logf(TEXT("%s expanded."),*ItemName);

	// Find this object's children.

	for(int Index = 0;Index < ChildItems.Num();Index++)
		if(!ChildItems(Index)->FoundChildren)
			ChildItems(Index)->InitChildren();

	unguard;
}

///////////////////////////////////////
// FTreeControlItem::Collapse
///////////////////////////////////////

void FTreeControlItem::Collapse()
{
	Expanded = 0;
}

///////////////////////////////////////
// FTreeControlItem::OnRightClick
///////////////////////////////////////

void FTreeControlItem::OnRightClick()
{
	guard(FTreeControlItem::OnRightClick);

	POINT	MousePos;

	// Get the mouse position.

	::GetCursorPos(&MousePos);

	// Create a popup menu.

	verify(TrackPopupMenu(GetSubMenu(LoadMenuIdX(hInstance,IDM_MainMenu),2),0,MousePos.x,MousePos.y,0,TreeControl->OwnerWindow->hWnd,NULL));

	unguard;
}

///////////////////////////////////////
// FTreeControlItem::GetInfo
///////////////////////////////////////

void FTreeControlItem::GetInfo(TVITEM& ItemInfo)
{
	guard(FTreeControlItem::GetInfo);

	ItemInfo.mask = TVIF_TEXT | TVIF_PARAM | TVIF_CHILDREN;
	ItemInfo.pszText = (TCHAR*) *ItemName;
	ItemInfo.lParam = (long) this;

	// Count this object's children.

	ItemInfo.cChildren = 0;

	for(FObjectIterator It;It;++It)
	{
		if(MainWindow->ObjectFilter == FILTER_AllClasses)
		{
			UClass*	Class = Cast<UClass>(*It);

			if(Class && Class->GetSuperClass() == Object)
			{
				ItemInfo.cChildren = 1;
				break;
			}
		}
		else if(MainWindow->ObjectFilter == FILTER_AllActors)
		{
			AActor*	Actor = Cast<AActor>(*It);

			if(Cast<ALevelInfo>(Object) && Actor && Actor->Level == Object)
			{
				ItemInfo.cChildren = 1;
				break;
			}
		}
		else
		{
			if(It->GetOuter() == Object)
			{
				ItemInfo.cChildren = 1;
				break;
			}
		}
	}

	unguard;
}

///////////////////////////////////////
// FTreeControlItem::InitChildren
///////////////////////////////////////

void FTreeControlItem::InitChildren()
{
	guard(FTreeControlItem::InitChildren);

	if(FoundChildren)
		return;

	// Find all child objects.

	for(FObjectIterator It;It;++It)
	{
		if(MainWindow->ObjectFilter == FILTER_AllClasses)
		{
			UClass*	Class = Cast<UClass>(*It);

			if(Class && Class->GetSuperClass() == Object)
				AddChild(new FTreeControlItem(TreeControl,*It));
		}
		else if(MainWindow->ObjectFilter == FILTER_AllActors)
		{
			AActor*	Actor = Cast<AActor>(*It);

			if(Cast<ALevelInfo>(Object) && Actor && Actor->Level == Object)
				AddChild(new FTreeControlItem(TreeControl,*It));
		}
		else
		{
			if(It->GetOuter() == Object)
				AddChild(new FTreeControlItem(TreeControl,*It));
		}
	}

	FoundChildren = 1;

	unguardf((TEXT("%s"),Object->GetFullName()));
}

///////////////////////////////////////
// FTreeControlItem::AddChild
///////////////////////////////////////

void FTreeControlItem::AddChild(FTreeControlItem* NewItem)
{
	guard(FTreeControlItem::AddChild);

	TV_INSERTSTRUCT	InsertInfo;

	// Add the item to the array of child items.

	ChildItems.AddItem(NewItem);

	// Set the item's parent item to this.

	NewItem->ParentItem = this;

	// Get the item information.

	InsertInfo.hParent = ItemHandle;
	InsertInfo.hInsertAfter = TVI_SORT;

	NewItem->GetInfo(InsertInfo.item);

	// Send a message to the tree control.

	NewItem->ItemHandle = (HTREEITEM) SendMessageX(TreeControl->hWnd,TVM_INSERTITEM,0,(LPARAM) &InsertInfo);

	// Notify the item that it has been created.

	NewItem->Created();

	unguard;
}


//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
// WTreeControl
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////


////////////////////////////////////////
// WTreeControl constructors
////////////////////////////////////////

WTreeControl::WTreeControl(WWindow* Owner,int InId) :
	WControl(Owner,InId,SuperProc)
{
}

////////////////////////////////////////
// WTreeControl::OpenWindow
////////////////////////////////////////

void WTreeControl::OpenWindow(UBOOL Visible,UBOOL ShowLines,FRect Rect)
{
	guard(WTreeControl::OpenWindow);

	PerformCreateWindowEx(0,TEXT("TreeView"),WS_CHILD | WS_BORDER | (ShowLines ? TVS_HASLINES | TVS_LINESATROOT | TVS_HASBUTTONS : 0) | (Visible ? WS_VISIBLE : 0),Rect.Min.X,Rect.Min.Y,Rect.Max.X - Rect.Min.X,Rect.Max.Y - Rect.Min.Y,(HWND) *OwnerWindow,(HMENU) ControlId,hInstance);
	SendMessageX(hWnd,WM_SETFONT,(WPARAM) GetStockObject(DEFAULT_GUI_FONT),MAKELPARAM(0,0));

	unguardf((TEXT("Visible=%u, ShowLines=%u, Rect=(%u,%u,%u,%u)"),(int) Visible,(int) ShowLines,Rect.Min.X,Rect.Min.Y,Rect.Max.X,Rect.Max.Y));
}

////////////////////////////////////////
// WTreeControl::AddItem
////////////////////////////////////////

void WTreeControl::AddItem(FTreeControlItem* NewItem)
{
	guard(WTreeControl::AddItem);

	TV_INSERTSTRUCT	InsertInfo;

	check(hWnd);

	// Add the item to the array of child items.

	RootItems.AddItem(NewItem);

	// Set the item's parent item to this.

	NewItem->ParentItem = NULL;

	// Get the item information.

	InsertInfo.hParent = NULL;
	InsertInfo.hInsertAfter = TVI_SORT;

	NewItem->GetInfo(InsertInfo.item);

	// Send a message to the tree control.

	NewItem->ItemHandle = (HTREEITEM) SendMessageX(hWnd,TVM_INSERTITEM,0,(LPARAM) &InsertInfo);

	// Notify the item that it has been created.

	NewItem->Created();

	unguard;
}

///////////////////////////////////////
// WTreeControl::Empty
///////////////////////////////////////

void WTreeControl::Empty()
{
	guard(WTreeControl::Empty);

	check(hWnd);

	// Clear the tree control.

	SendMessageX(hWnd,TVM_DELETEITEM,0,(long) TVI_ROOT);

	// Clear the RootItems array.

	for(int Index = 0;Index < RootItems.Num();Index++)
		delete RootItems(Index);

	RootItems.Empty();

	// Clear the selected item.

	SelectedItem = NULL;

	unguard;
}

///////////////////////////////////////
// WTreeControl::SelectionChanged
///////////////////////////////////////

void WTreeControl::SelectionChanged(FTreeControlItem* NewSelection)
{
	SelectedItem = NewSelection;
}

///////////////////////////////////////
// WTreeControl::GetSelected
///////////////////////////////////////

FTreeControlItem* WTreeControl::GetSelected()
{
	return SelectedItem;
}


//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
// WPackageToolWindow
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////


///////////////////////////////////////
// WPackageToolWindow constructor
///////////////////////////////////////

WPackageToolWindow::WPackageToolWindow() :
	TreeControl(this,0)
{
}

///////////////////////////////////////
// WPackageToolWindow::OpenWindow
///////////////////////////////////////

void WPackageToolWindow::OpenWindow()
{
	guard(WPackageToolWindow::OpenWindow);

	// Create the main window.

	PerformCreateWindowEx(0,TEXT("Package Tool"),WS_OVERLAPPEDWINDOW | WS_VISIBLE,100,100,512,384,NULL,LoadMenuIdX(hInstance,MAKEINTRESOURCEX(IDM_MainMenu)),hInstance);

	// Create the tree control.

	TreeControl.OpenWindow(TRUE,TRUE,GetClientRect());

	// Populate the tree control.

	Refresh();

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::WndProc
///////////////////////////////////////

LONG WPackageToolWindow::WndProc(UINT Message,UINT WParam,LONG LParam)
{
	guard(WPackageToolWindow::WndProc);

	if(Message == WM_NOTIFY)
	{
		OnNotify(WParam,(NMHDR*) LParam);

		return WWindow::WndProc(Message,WParam,LParam);
	}
	else
		return WWindow::WndProc(Message,WParam,LParam);

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::OnNotify
///////////////////////////////////////

void WPackageToolWindow::OnNotify(int InControlId,NMHDR* NotifyHeader)
{
	guard(WPackageToolWindow::OnNotify);

	if(TreeControl.ControlId == InControlId)
	{
		if(NotifyHeader->code == TVN_ITEMEXPANDEDA)
		{
			NMTREEVIEW* NotifyInfo = (NMTREEVIEW*) NotifyHeader;

			// Forward the expanded notification to the relevant FTreeControlItem.

			if(NotifyInfo->itemNew.state & TVIS_EXPANDED)
				((FTreeControlItem*) NotifyInfo->itemNew.lParam)->Expand();
			else
				((FTreeControlItem*) NotifyInfo->itemNew.lParam)->Collapse();
		}
		else if(NotifyHeader->code == NM_RCLICK)
		{
			TVHITTESTINFO	HitTest;
			TVITEM			Item;
			FPoint			MousePos;

			// Get the mouse position.

			MousePos = TreeControl.GetCursorPos();

			// Determine which item was right clicked on.

			HitTest.pt.x = MousePos.X;
			HitTest.pt.y = MousePos.Y;

			SendMessageX((HWND) TreeControl,TVM_HITTEST,0,(LPARAM) &HitTest);

			if(HitTest.hItem)
			{
				// Get information about the item that was right clicked on.

				appMemzero(&Item,sizeof(Item));
				Item.mask = TVIF_PARAM;
				Item.hItem = HitTest.hItem;

				SendMessageX((HWND) TreeControl,TVM_GETITEM,0,(LPARAM) &Item);

				// Select the item.

				SendMessageX((HWND) TreeControl,TVM_SELECTITEM,TVGN_CARET,(LPARAM) HitTest.hItem);

				// Notify the item.

				((FTreeControlItem*) Item.lParam)->OnRightClick();
			}
		}
		else if(NotifyHeader->code == TVN_SELCHANGEDA)
		{
			NMTREEVIEW*	NotifyInfo = (NMTREEVIEW*) NotifyHeader;

			// Forward the selection changed notification to the tree control.

			TreeControl.SelectionChanged((FTreeControlItem*) NotifyInfo->itemNew.lParam);
		}
	}

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::OnCommand
///////////////////////////////////////

void WPackageToolWindow::OnCommand(int Command)
{
	guard(WPackageToolWindow::OnCommand);

	if(Command == ID_Exit)
		GIsRequestingExit = 1;
	else if(Command == ID_LoadPackage)
	{
		guard(LoadPackage);

		ANSICHAR		Filename[512];
		OPENFILENAMEA	OpenFile;

		// Setup the OpenFile struct.

		appMemzero(&OpenFile,sizeof(OpenFile));
		appMemzero(Filename,sizeof(Filename));

		OpenFile.lStructSize = sizeof(OpenFile);
		OpenFile.hInstance = hInstance;
		OpenFile.hwndOwner = hWnd;
		OpenFile.lpstrFilter = "Unreal Packages\0*.u;*.unr;*.umx;*.usx;*.usa;*.utx\0";
		OpenFile.lpstrFile = Filename;
		OpenFile.nMaxFile = 512;
		OpenFile.lpstrInitialDir = appToAnsi(appBaseDir());
		OpenFile.lpstrTitle = "Load Package";

		// Display the open dialog.

		if(GetOpenFileNameA(&OpenFile))
			UObject::LoadPackage(NULL,appFromAnsi(OpenFile.lpstrFile),LOAD_NoFail);

		// Repopulate the tree control.

		Refresh();

		unguard;
	}
	else if(Command == ID_SavePackage)
	{
		guard(SavePackage);

		FTreeControlItem*	SelectedItem;
		UPackage*			Package = NULL;

		// Determine which package the user wishes to save.

		SelectedItem = TreeControl.GetSelected();

		if(SelectedItem)
			Package = Cast<UPackage>(SelectedItem->Object);

		if(Package && !Package->GetOuter())
		{
			UObject* Obj = NULL;

			// Find an object in the package being saved.

			for(FObjectIterator It;It;++It)
			{
				if(It->GetOuter() == Package)
				{
					Obj = *It;
					break;
				}
			}

			// Make sure the package isn't empty.

			if(!Obj)
			{
				MessageBox(hWnd,TEXT("That package is empty."),TEXT("Save failed"),MB_OK);

				return;
			}

			// Make sure the package was loaded from a file.

			if(!Obj->GetLinker())
			{
				MessageBox(hWnd,TEXT("That package is not savable."),TEXT("Save failed"),MB_OK);

				return;
			}

			// Get the filename to save the package to.

			ANSICHAR		Filename[512];
			TCHAR			Temp[512];
			OPENFILENAMEA	SaveFile;

			// Find an existing package file, and default to it's file name.

			appFindPackageFile(Package->GetName(),NULL,Temp);
			strcpy(Filename,appToAnsi(Temp));

			appMemzero(&SaveFile,sizeof(SaveFile));

			SaveFile.lStructSize = sizeof(SaveFile);
			SaveFile.hInstance = hInstance;
			SaveFile.hwndOwner = hWnd;
			SaveFile.lpstrFilter = "Unreal Packages\0*.u;*.unr;*.umx;*.usx;*.usa;*.utx\0";
			SaveFile.lpstrFile = Filename;
			SaveFile.nMaxFile = 512;
			SaveFile.lpstrInitialDir = appToAnsi(appBaseDir());
			SaveFile.lpstrTitle = "Save Package";

			// Save the package.

			if(GetSaveFileNameA(&SaveFile))
				UObject::SavePackage(Package,NULL,0xffffffff,appFromAnsi(Filename));
		}
		else
			MessageBox(hWnd,TEXT("You have not selected a package to save."),TEXT("Save failed"),MB_OK);

		unguard;
	}
	else if(Command == ID_Refresh)
	{
		guard(Refresh);

		// Refresh the tree view.

		Refresh();

		unguard;
	}
	else if(Command == ID_ShowLog)
		GLogWindow->Show(1);
	else if(Command == ID_ShowObject)
	{
		guard(ShowObject);

		// Open an object properties window.

		FTreeControlItem*	Item = (FTreeControlItem*) TreeControl.GetSelected();

		if(Item)
		{
			WObjectProperties*	PropertiesWindow = new WObjectProperties(TEXT("Object properties"),0,TEXT(""),NULL,1);

			PropertiesWindow->OpenWindow(hWnd);
			PropertiesWindow->Root.SetObjects((UObject**) &Item->Object,1);
			PropertiesWindow->Show(1);
		}
		else
			MessageBox(hWnd,TEXT("You do not have an object selected."),TEXT("Couldn't open properties window"),MB_OK);

		unguard;
	}
	else if(Command == ID_CreateObject)
	{
		guard(CreateObject);

		FTreeControlItem*	SelectedItem = TreeControl.GetSelected();

		WCreateDialog	CreationDialog(this,SelectedItem ? SelectedItem->Object : NULL);

		// Show the create dialog.

		if(CreationDialog.DoModal(hInstance))
			Refresh();

		unguard;
	}
	else if(Command == ID_DestroyObject)
	{
		guard(DestroyObject);

		FTreeControlItem*	SelectedItem = TreeControl.GetSelected();

		if(SelectedItem)
		{
			if(SelectedItem->Object->IsReferenced(SelectedItem->Object,0,0))
				if(!MessageBox(hWnd,TEXT("That object is referenced.  Destroying it could cause unwanted side effects.  Are you sure you want to destroy it?"),TEXT("Warning"),MB_YESNO | MB_ICONWARNING))
					return;

			// Destroy the selected object and refresh the tree.

			delete SelectedItem->Object;

			Refresh();
		}

		unguard;
	}
	else if(Command == ID_ImportObject)
	{
		guard(ImportObject);

		FTreeControlItem*	SelectedItem = TreeControl.GetSelected();

		if(SelectedItem)
		{
			WImportDialog	ImportDialog(this,SelectedItem->Object);

			// Show the import dialog.

			if(ImportDialog.DoModal(hInstance))
				Refresh();
		}
		else
			MessageBox(hWnd,TEXT("You must select the object you wish to import the object under."),TEXT("Importing failed"),MB_OK);

		unguard;
	}
	else if(Command == ID_ExportObject)
	{
		guard(ExportObject);

		// Determine the object the user wants to export.

		FTreeControlItem*	Item = (FTreeControlItem*) TreeControl.GetSelected();

		if(Item)
		{
			// Get the filename to export the object to.

			UObject*		Object = Item->Object;
			ANSICHAR		Filename[512];
			OPENFILENAMEA	SaveFile;

			sprintf(Filename,"%s",appToAnsi(Object->GetName()));
			appMemzero(&SaveFile,sizeof(SaveFile));

			SaveFile.lStructSize = sizeof(SaveFile);
			SaveFile.hInstance = hInstance;
			SaveFile.hwndOwner = hWnd;
			SaveFile.lpstrFilter = "*.*\0*.*\0";
			SaveFile.lpstrFile = Filename;
			SaveFile.nMaxFile = 512;
			SaveFile.lpstrInitialDir = appToAnsi(appBaseDir());
			SaveFile.lpstrTitle = "Export Object";

			// Export the object.

			if(GetSaveFileNameA(&SaveFile))
			{
				UExporter*	Exporter = UExporter::FindExporter(Object,(const TCHAR*) appFromAnsi(Filename + SaveFile.nFileExtension));

				if(Exporter)
				{
					if(!UExporter::ExportToFile(Object,Exporter,(const TCHAR*) appFromAnsi(Filename)))
						MessageBox(hWnd,TEXT("Export failed."),TEXT("Export failed"),MB_OK);
					else
					{
						TCHAR	Temp[512];

						appSprintf(Temp,TEXT("Successfully exported %s to %s."),Object->GetFullName(),appFromAnsi(Filename));

						MessageBox(hWnd,Temp,TEXT("Export succeeded"),MB_OK);
					}
				}
				else
					MessageBox(hWnd,TEXT("Couldn't find an appropriate exporter for that file type."),TEXT("Export failed"),MB_OK);
			}
		}
		else
			MessageBox(hWnd,TEXT("You have not selected an object to export."),TEXT("Export failed"),MB_OK);

		unguard;
	}
	else if(Command == ID_Filter_AllObjects)
	{
		if(ObjectFilter != FILTER_AllObjects)
		{
			// Change the filter and refresh the tree.

			ObjectFilter = FILTER_AllObjects;

			Refresh();
		}
	}
	else if(Command == ID_Filter_AllClasses)
	{
		if(ObjectFilter != FILTER_AllClasses)
		{
			// Change the filter and refresh the tree.

			ObjectFilter = FILTER_AllClasses;

			Refresh();
		}
	}
	else if(Command == ID_Filter_AllActors)
	{
		if(ObjectFilter != FILTER_AllActors)
		{
			// Change the filter and refresh the tree.

			ObjectFilter = FILTER_AllActors;

			Refresh();
		}
	}
	else if(Command == ID_CollectGarbage)
	{
		// Collect the garbage and refresh the tree.

		UObject::CollectGarbage(RF_Native | RF_Standalone);

		Refresh();
	}

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::OnClose
///////////////////////////////////////

void WPackageToolWindow::OnClose()
{
	guard(WPackageToolWindow::OnClose);

	// Exit.

	GIsRequestingExit = 1;

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::OnPaint
///////////////////////////////////////

void WPackageToolWindow::OnPaint()
{
	guard(WPackageToolWindow::OnPaint);

	if(GetUpdateRect(*this,NULL,0))
	{
		PAINTSTRUCT	PS;
		HDC			DC = BeginPaint(*this,&PS);
		float		Y = -GetScrollPos(hWnd,SB_VERT);

		FRect		Rect = GetClientRect();

		// Clear the background to the dialog background color.

		FillRect(DC,Rect,(HBRUSH) COLOR_3DFACE - 1);

		EndPaint(*this,&PS);
	}

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::OnSize
///////////////////////////////////////

void WPackageToolWindow::OnSize(DWORD Flags,int NewWidth,int NewHeight)
{
	guard(WPackageToolWindow::OnSize);

	// Resize the tree control.

	SetWindowPos(TreeControl.hWnd,HWND_TOP,0,0,NewWidth,NewHeight,0);

	unguardf((TEXT("(%u,%u)"),NewWidth,NewHeight));
}

///////////////////////////////////////
// WPackageToolWindow::OnSetCursor
///////////////////////////////////////

int WPackageToolWindow::OnSetCursor()
{
	guard(WPackageToolWindow::OnSetCursor);

	// Set the arrow cursor.

	SetCursor(LoadCursorIdX(NULL,MAKEINTRESOURCE(IDC_ARROW)));

	return 0;

	unguard;
}

///////////////////////////////////////
// WPackageToolWindow::Refresh
///////////////////////////////////////

void WPackageToolWindow::Refresh()
{
	guard(WPackageToolWindow::Refresh);

	// Empty the tree control.

	TreeControl.Empty();

	// Add the root objects to the tree.

	if(ObjectFilter == FILTER_AllClasses)
		TreeControl.AddItem(new FTreeControlItem(&TreeControl,UObject::StaticClass()));
	else
	{
		for(FObjectIterator It;It;++It)
		{
			if(ObjectFilter == FILTER_AllActors)
			{
				if(Cast<ALevelInfo>(*It))
					TreeControl.AddItem(new FTreeControlItem(&TreeControl,*It));
			}
			else
			{
				if(!It->GetOuter())
					TreeControl.AddItem(new FTreeControlItem(&TreeControl,*It));
			}
		}
	}

	unguard;
}


//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
// WImportDialog
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////
// WImportDialog constructor
///////////////////////////////////////

WImportDialog::WImportDialog(WWindow* InOwner,UObject* InOuter) :
	WDialog(TEXT("ImportDialog"),IDD_ImportDialog,InOwner),
	ClassList(this,IDC_Class),
	NameEdit(this,IDC_Name),
	OuterEdit(this,IDC_Outer),
	FilenameEdit(this,IDC_Filename),
	FileBrowseButton(this,IDC_BrowseFiles,FDelegate(this,(TDelegate) OnBrowseFiles)),
	OKButton(this,IDOK,FDelegate(this,(TDelegate) OnOK)),
	CancelButton(this,IDCANCEL,FDelegate(this,(TDelegate) OnCancel))
{
	guard(WImportDialog::WImportDialog);

	check(InOuter);

	Outer = InOuter;

	unguard;
}

///////////////////////////////////////
// WImportDialog::OnInitDialog
///////////////////////////////////////

void WImportDialog::OnInitDialog()
{
	guard(WImportDialog::OnInitDialog);

	WDialog::OnInitDialog();

	// Setup the outer text box.

	OuterEdit.SetText(Outer->GetFullName());

	// Setup the class list.

	for(TObjectIterator<UClass> It;It;++It)
		if(!(It->ClassFlags & CLASS_Abstract) && !(It->ClassFlags & CLASS_NoUserCreate))
			ClassList.AddString(It->GetName());

	unguard;
}

///////////////////////////////////////
// WImportDialog::OnBrowseFiles
///////////////////////////////////////

void WImportDialog::OnBrowseFiles()
{
	guard(WImportDialog::OnBrowseFiles);

	ANSICHAR		Filename[512];
	OPENFILENAMEA	OpenFile;

	// Setup the OpenFile struct.

	appMemzero(&OpenFile,sizeof(OpenFile));
	appMemzero(Filename,sizeof(Filename));

	OpenFile.lStructSize = sizeof(OpenFile);
	OpenFile.hInstance = hInstance;
	OpenFile.hwndOwner = hWnd;
	OpenFile.lpstrFilter = "All Files\0*.*\0";
	OpenFile.lpstrFile = Filename;
	OpenFile.nMaxFile = 512;
	OpenFile.lpstrInitialDir = appToAnsi(appBaseDir());
	OpenFile.lpstrTitle = "Import Object";

	// Display the open dialog.

	if(GetOpenFileNameA(&OpenFile))
		FilenameEdit.SetText(appFromAnsi(Filename));

	unguard;
}

///////////////////////////////////////
// WImportDialog::OnOK
///////////////////////////////////////

void WImportDialog::OnOK()
{
	guard(WImportDialog::OnOK);

	// Make sure the user has selected a class.

	if(ClassList.GetText() == TEXT(""))
	{
		MessageBox(hWnd,TEXT("You must have a class selected."),TEXT("Importing failed"),MB_OK);

		return;
	}

	// Import the object.

	UClass*		ImportClass = FindObjectChecked<UClass>(ANY_PACKAGE,*ClassList.GetText());
	FString		NameText = NameEdit.GetText();
	UObject*	Imported = UFactory::StaticImportObject(ImportClass,Outer,(NameText == TEXT("") ? NAME_None : FName(*NameText)),0,*FilenameEdit.GetText());

	if(!Imported)
		MessageBox(hWnd,TEXT("Import failed."),TEXT("Import failed"),MB_OK);
	else
		GLog->Logf(TEXT("Imported: %s"),Imported->GetFullName());

	// Close this dialog box.

	EndDialog(1);

	unguard;
}

///////////////////////////////////////
// WImportDialog::OnCancel
///////////////////////////////////////

void WImportDialog::OnCancel()
{
	guard(WImportDialog::OnCancel);

	// Close this dialog box.

	EndDialog(0);

	unguard;
}


//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
// WCreateDialog
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////
// WCreateDialog constructor
///////////////////////////////////////

WCreateDialog::WCreateDialog(WWindow* InOwner,UObject* InOuter) :
	WDialog(TEXT("CreateDialog"),IDD_CreateDialog,InOwner),
	ClassList(this,IDC_Class),
	NameEdit(this,IDC_Name),
	OuterEdit(this,IDC_Outer),
	OKButton(this,IDOK,FDelegate(this,(TDelegate) OnOK)),
	CancelButton(this,IDCANCEL,FDelegate(this,(TDelegate) OnCancel))
{
	guard(WCreateDialog::WCreateDialog);

	Outer = InOuter;

	unguard;
}

///////////////////////////////////////
// WCreateDialog::OnInitDialog
///////////////////////////////////////

void WCreateDialog::OnInitDialog()
{
	guard(WCreateDialog::OnInitDialog);

	WDialog::OnInitDialog();

	// Setup the outer text box.

	OuterEdit.SetText(Outer ? Outer->GetFullName() : TEXT("None"));

	// Setup the class list.

	for(TObjectIterator<UClass> It;It;++It)
		if(!(It->ClassFlags & CLASS_Abstract) && !(It->ClassFlags & CLASS_NoUserCreate))
			ClassList.AddString(It->GetName());

	unguard;
}

///////////////////////////////////////
// WCreateDialog::OnOK
///////////////////////////////////////

void WCreateDialog::OnOK()
{
	guard(WCreateDialog::OnOK);

	// Make sure the user has selected a class.

	if(ClassList.GetText() == TEXT(""))
	{
		MessageBox(hWnd,TEXT("You must have a class selected."),TEXT("Creation failed"),MB_OK);

		return;
	}

	// Import the object.

	UClass*		CreateClass = FindObjectChecked<UClass>(ANY_PACKAGE,*ClassList.GetText());
	FString		NameText = NameEdit.GetText();
	UObject*	Created = UObject::StaticConstructObject(CreateClass,Outer,*NameText);

	if(!Created)
		MessageBox(hWnd,TEXT("Creation failed."),TEXT("Creation failed"),MB_OK);
	else
		GLog->Logf(TEXT("Created: %s"),Created->GetFullName());

	// Close this dialog box.

	EndDialog(1);

	unguard;
}

///////////////////////////////////////
// WCreateDialog::OnCancel
///////////////////////////////////////

void WCreateDialog::OnCancel()
{
	guard(WCreateDialog::OnCancel);

	// Close this dialog box.

	EndDialog(0);

	unguard;
}


///////////////////////////////////////
// WinMain
///////////////////////////////////////

int WINAPI WinMain(HINSTANCE Instance,HINSTANCE PrevInstance,LPSTR CommandLine,int ShowCommand)
{
	INT	ErrorLevel = 0;

	GIsStarted = 1;
	hInstance = Instance;

	try
	{
		GIsGuarded = 1;
		GIsRunning = 1;

		// Initialize the engine.

		guard(Init);

		appSprintf(Log.Filename,TEXT("%sPackageTool.log"),appBaseDir());

		appInit(TEXT("Unreal"),appFromAnsi(CommandLine),&Malloc,&Log,&Error,&Warn,&FileManager,FConfigCacheIni::Factory,1);

		GIsClient = GIsServer = GIsEditor = 1;
		GIsScriptable = GLazyLoad = 0;

		// Load the editor package.

		UObject::LoadPackage(NULL,TEXT("Editor.u"),LOAD_NoFail);

		// Initialize the common controls lib.

		InitCommonControls();

		// Initialize the windowing system.

		InitWindowing();

		IMPLEMENT_WINDOWCLASS(WPackageToolWindow,0);
		IMPLEMENT_WINDOWCLASS(WImportDialog,0);
		IMPLEMENT_WINDOWCLASS(WCreateDialog,0);
		IMPLEMENT_WINDOWSUBCLASS(WTreeControl,WC_TREEVIEW);

		unguard;

		// Create the log window.

		GLogWindow = new WLog(Log.Filename,Log.LogAr,TEXT("GameLog"));
		GLogWindow->OpenWindow(FALSE,FALSE);

		// Create the main window.

		guard(CreateMainWindow);

		MainWindow = new WPackageToolWindow;
		MainWindow->OpenWindow();

		unguard;

		// The main loop.

		DWORD	ThreadId = GetCurrentThreadId();
		HANDLE	hThread = GetCurrentThread();

		while(GIsRunning && !GIsRequestingExit)
		{
			guard(MessagePump);

			// Handle all incoming messages.

			MSG	Msg;

			while(PeekMessageX(&Msg,NULL,0,0,PM_REMOVE))
			{
				if(Msg.message == WM_QUIT)
					GIsRequestingExit = 1;

				guard(TranslateMessage);
				TranslateMessage(&Msg);
				unguardf((TEXT("%08X %i"),(int) Msg.hwnd,Msg.message));

				guard(DispatchMessage);
				DispatchMessageX(&Msg);
				unguardf((TEXT("%08X %i"),(int) Msg.hwnd,Msg.message));
			};

			unguard;
		};

		// Exit.

		delete MainWindow;

		delete GLogWindow;

		appPreExit();
		GIsGuarded = 0;
		GIsRunning = 0;
	}
	catch(...)
	{
		// Crashed.

		ErrorLevel = 1;
		GIsGuarded = 0;
		Error.HandleError();
	}

	appExit();
	GIsStarted = 0;

	return ErrorLevel;
}