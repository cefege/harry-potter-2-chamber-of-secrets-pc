// PackageTool.h
// Copyright (c) 1999, Andrew Scheidecker

#include <windows.h>
#include <commctrl.h>
#include <shlobj.h>
#include <io.h>
#include <direct.h>
#include <errno.h>
#include <stdio.h>
#include <sys/stat.h>

#include "Core.h"
#include "Engine.h"
#include "Window.h"

#include "FMallocWindows.h"
#include "FOutputDeviceFile.h"
#include "FOutputDeviceWindowsError.h"
#include "FFeedbackContextWindows.h"
#include "FFileManagerWindows.h"
#include "FConfigCacheIni.h"

#include "Resource.h"


///////////////////////////////////////
// FTreeControlItem
///////////////////////////////////////

class FTreeControlItem
{
public:

	HTREEITEM					ItemHandle;

	UBOOL						InTree;
	UBOOL						Expanded;

	FTreeControlItem*			ParentItem;
	TArray<FTreeControlItem*>	ChildItems;
	class WTreeControl*			TreeControl;

	UObject*					Object;
	FString						ItemName;

	UBOOL						FoundChildren;


	///////////////////////////////////////
	// Constructor/Destructor
	///////////////////////////////////////

	FTreeControlItem(class WTreeControl* InTreeControl,UObject* InObject);
	virtual ~FTreeControlItem();

	///////////////////////////////////////
	// Notifications
	///////////////////////////////////////

	virtual void Created();
	virtual void Expand();
	virtual void Collapse();
	virtual void OnRightClick();

	virtual void GetInfo(TVITEM& ItemInfo);

	///////////////////////////////////////
	// Utility functions
	///////////////////////////////////////

	void InitChildren();
	void AddChild(FTreeControlItem* NewItem);
};

///////////////////////////////////////
// WTreeControl
///////////////////////////////////////

class WTreeControl : public WControl
{
	DECLARE_WINDOWSUBCLASS(WTreeControl,WControl,PackageTool);

	FTreeControlItem*			SelectedItem;
	TArray<FTreeControlItem*>	RootItems;

	////////////////////////////////////////
	// Constructor
	////////////////////////////////////////

	WTreeControl(WWindow* Owner,int InId);

	////////////////////////////////////////
	// Utility functions
	////////////////////////////////////////

	void OpenWindow(UBOOL Visible,UBOOL ShowLines,FRect Rect);
	void AddItem(FTreeControlItem* NewItem);
	void Empty();
	FTreeControlItem* GetSelected();

	///////////////////////////////////////
	// Notifications
	///////////////////////////////////////

	void SelectionChanged(FTreeControlItem* NewSelection);
};

enum EFilterType
{
	FILTER_AllObjects,
	FILTER_AllClasses,
	FILTER_AllActors
};

///////////////////////////////////////
// WPackageToolWindow
///////////////////////////////////////

class WPackageToolWindow : public WWindow
{
	DECLARE_WINDOWCLASS(WPackageToolWindow,WWindow,PackageTool);

	WTreeControl	TreeControl;

	EFilterType		ObjectFilter;

	///////////////////////////////////////
	// Constructor
	///////////////////////////////////////

	WPackageToolWindow();

	///////////////////////////////////////
	// Message handlers
	///////////////////////////////////////

	virtual LONG WndProc(UINT Message,UINT WParam,LONG LParam);
	virtual void OnNotify(int InControlId,NMHDR* NotifyHeader);
	virtual void OnCommand(int Command);
	virtual void OnClose();
	virtual void OnPaint();
	virtual void OnSize(DWORD Flags,int NewWidth,int NewHeight);
	virtual int OnSetCursor();

	///////////////////////////////////////
	// Utility functions
	///////////////////////////////////////

	void Refresh();
	void OpenWindow();
};

///////////////////////////////////////
// WImportDialog
///////////////////////////////////////

class WImportDialog : public WDialog
{
	DECLARE_WINDOWCLASS(WImportDialog,WDialog,PackageTool);

	UObject*	Outer;

	WComboBox	ClassList;

	WEdit		NameEdit,
				OuterEdit,
				FilenameEdit;

	WCoolButton	FileBrowseButton,
				OKButton,
				CancelButton;

	///////////////////////////////////////
	// Constructor
	///////////////////////////////////////

	WImportDialog(WWindow* InOwner,UObject* InOuter);

	///////////////////////////////////////
	// Message handlers
	///////////////////////////////////////

	virtual void OnInitDialog();

	virtual void OnBrowseFiles();

	virtual void OnOK();
	virtual void OnCancel();
};

///////////////////////////////////////
// WCreateDialog
///////////////////////////////////////

class WCreateDialog : public WDialog
{
	DECLARE_WINDOWCLASS(WCreateDialog,WDialog,PackageTool);

	UObject*	Outer;

	WComboBox	ClassList;

	WEdit		NameEdit,
				OuterEdit;

	WCoolButton	OKButton,
				CancelButton;

	///////////////////////////////////////
	// Constructor
	///////////////////////////////////////

	WCreateDialog(WWindow* InOwner,UObject* InOuter);

	///////////////////////////////////////
	// Message handlers
	///////////////////////////////////////

	virtual void OnInitDialog();

	virtual void OnOK();
	virtual void OnCancel();
};
