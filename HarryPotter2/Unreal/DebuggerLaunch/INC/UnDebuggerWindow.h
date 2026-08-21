/*=============================================================================
	UnDebuggerWindow.h: Debugger Windows code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Lucas Alonso.
=============================================================================*/

// All this code is in need of a major cleanup, since there's alot of stuff
// leftover from the various partial rewrites it underwent.

#define USE_INI TEXT("hp.ini")
#define USE_NAME TEXT("hp")

class UDebuggerWindow;
class FCallStack;
class FStackNode;
class FBreakpointManager;
class FBreakpoint;
class FDebuggerState;
class WDlgObject;

enum UserAction { UA_StepInto, 
				  UA_StepOver,
				  UA_StepOverStack,
				  UA_StepOut, 
				  UA_RunToCursor,
				  UA_Go,
				  UA_Exit,
				};

class FDebuggerState
{
public:
	virtual void Process();
	virtual void UpdateStackInfo( FStackNode* CNode );
	virtual void HandleInput( UserAction UserInput ) {};
	void SetParent( UDebuggerWindow* _Parent ) { Parent = _Parent; }
protected:
	UDebuggerWindow* Parent;

	UObject*	CurrentObject;
	FFrame*		CurrentStack;
	UClass*		CurrentClass;
	INT			CurrentLine;
	INT			CurrentPos;
	FString		CurrentInfo;
	INT			CurrentDepth;
	FStackNode* CurrentStackNode;
};


class DSWaitForInput : public FDebuggerState
{
protected:
	void Process();
	void HandleInput( UserAction UserInput );
	void ContinueExecution();
	void PumpMessages();
	
	UBOOL bContinue;

	friend UDebuggerWindow;
};

class DSWaitForCondition : public FDebuggerState
{
protected:
	virtual void Process();
	virtual UBOOL EvaluateCondition();
	friend UDebuggerWindow;
};

class DSRunToCursor : public DSWaitForCondition
{
public:
	DSRunToCursor( int InPos, int SDepth );
	UBOOL EvaluateCondition();
	INT ExpectedPos;
	INT EvalDepth;
};

class DSStepOver : public DSWaitForCondition
{
public:
	DSStepOver( int InPos, int SDepth );
protected:
	UBOOL	EvaluateCondition();
	INT		StartPos;
	INT		NextPos;
	INT		EvalDepth;
};

class DSIdleState : public FDebuggerState
{
	virtual void Process() {}
};



class DSStepOut : public DSWaitForCondition
{
public:
	DSStepOut( INT SDepth )
	{
		EvalDepth = SDepth;
	}
protected:
	UBOOL EvaluateCondition();
	friend UDebuggerWindow;
	FFrame*  OldStack;
	INT EvalDepth;
};

class DSStepOverStack : public DSWaitForCondition
{
public:
	DSStepOverStack( INT SDepth )
	{
		EvalDepth = SDepth;
	}
protected:
	UBOOL EvaluateCondition();
	friend UDebuggerWindow;
	FFrame*  OldStack;
	INT EvalDepth;
};

class WDebugEdit : public WRichEdit
{
public:
	WDebugEdit( WWindow* PWin ) : WRichEdit( PWin )
	{
	}
	UDebuggerWindow* Parent;
	void OnRightButtonDown();
};


class FStackNode
{
public:
	FStackNode( UObject* Debugee, FFrame* Stack, int LineNumber, int InputPos, FString AdditionalInfo )
	{
		Update( Debugee, Stack, LineNumber, InputPos, AdditionalInfo );
	}

	void Update( UObject* Debugee, FFrame* Stack, int LineNumber, int InputPos, FString AdditionalInfo )
	{
		Object = Debugee;
		StackNode = Stack;
		Line = LineNumber;
		Pos = InputPos;
		Info = AdditionalInfo;
	}

	void GetStatus( UObject** Obj, FFrame** Stack, UClass** cClass, INT* cLine, INT* cPos, FString* cInfo )
	{
		if ( Obj )
			*Obj = Object;
		if ( Stack )
			*Stack = StackNode;
		if ( cClass )
			*cClass = Class;
		if ( cPos )
			*cPos = Pos;
		if ( cLine )
			*cLine = Line;
		if ( cInfo )
			*cInfo = Info;
	}

	UObject* Object;
	UClass* Class;
	FFrame* StackNode;
	INT Line;
	INT Pos;
	FString Info;
};

class FCallStack
{
public:
	FCallStack( UDebuggerWindow* _Parent )
	{
		Parent = _Parent;
		StackDepth = 0;
	}
	
	void UpdateStack( UObject* Debugee, FFrame* FStack, int LineNumber, int InputPos, FString AdditionalInfo );

	FStackNode* TopNode() const
	{
		if ( Stack.Num() == 0 )
			return NULL;
		return Stack(StackDepth-1);
	}

	FStackNode* PreviousNode() const
	{
		if ( Stack.Num() < 2 )
			return NULL;
		return Stack(StackDepth-2);
	}

	INT Num()
	{
		return Stack.Num();
	}

	FStackNode* GetNode( INT i )
	{
		return Stack(i);
	}

	INT GetStackDepth()
	{
		return StackDepth;
	}

private:
	TArray<FStackNode*> Stack;
	INT StackDepth;
	UDebuggerWindow* Parent;
};



class UDebuggerWindow : public WWindow, public FControlSnoop, public UDebugger
{
public:
	// Constructors
	UDebuggerWindow();

	// UDebugger interface
	void DebugInfo( UObject* Debugee, FFrame* Stack, FString InfoType, int LineNumber, int InputPos );
	void LoadEditPackages();
	void LoadIni();
	void RebuildTree();

	UClass* CurrentLoadedClass;
	UClass* CurrentSelectedClass;
	
	// Debugger States state stuff
	void ChangeState( FDebuggerState* NewState );
	void ProcessPendingState();
	void ShowDebugContextMenu();
	void SetStatus( const TCHAR* Status );
	void StackChanged();
	void UpdateInterface( FStackNode* CNode );
	void ClearInterface();
	void SaveBreakpoints();
	void LoadBreakpoints();

	FCallStack* GetCallStack()
	{
		return CallStack;
	}
	void Shutdown();
	void SetDebuggerLine( FStackNode* CNode );
	FString GetPropText( UProperty* Prop, void* PropAddr );

	FBreakpointManager* BreakpointManager;

	// Variable watch window
	void RefreshWatch( FStackNode* CNode );
	
	void SetClass( UClass* Class, UBOOL bForce=FALSE );
	TArray<FStackNode*> VisibleNodes;
	TMap<FString,FString> ScriptHash;
	TMap<FString,FString> HighlightHash;
	UBOOL IsDebugging;
	UBOOL NeedsClear;
	UBOOL bClosing;
	UBOOL bShowOnlyActorClasses;
	FStackNode* WatchNode;
	
private:
	
	FString ColorConfig;
	FDebuggerState* CurrentState;
	FDebuggerState* PendingState;

	FCallStack* CallStack;
	
	// Richedit box stuff
	POINT ContextClick;
	UBOOL LoadClassText( UClass* Class );
	int FindPos( const TCHAR* InText, int Line );

	void ScrollToLine( INT Line, INT bCenter=0 );

	void FormatRichText( UClass* TextClass, const TCHAR* Item, FOutputDevice& Ar );
	void RefreshRichText( UClass* TClass );

	void InsertProperty( UProperty* Prop, UObject* Obj, FFrame* Stack, UBOOL bIsLocal );

	TArray<UProperty*> LoadedProperties;
	TArray<void*> LoadedPropertyData;
	int ItemCtr;

	// Smart debug highlighting
	FString Parse_MatchToken( char*& Buffer, int& iChar, UBOOL bReverse );
	UBOOL   Parse_MatchChar( char*& Buffer, int& iChar, char Match, UBOOL bReverse );
	UBOOL   Parse_MatchParenBlocks( char*& Buffer, int& iChar, UBOOL bReverse, int InitialBalance=0 );
	UBOOL   Parse_SkipWhiteSpace( char*& Buffer, int& iChar, UBOOL bReverse );
	
	FString StatusBarText;

	// WWindow related stuff;

	UDebuggerWindow( FName InPersistentName, WWindow* InOwnerWindow );
	DECLARE_WINDOWCLASS(UDebuggerWindow,WWindow,DebuggerLaunch)

	void CloseObjectWindow();
		
	void PositionChildControls();
	
	void OnSize( DWORD Flags, INT NewX, INT NewY );
	void OnCreate();
	void OnDestroy();
	void OpenWindow();
	void OnPaint();
	void OnCommand( INT Command );
	void OnKeyUp( WPARAM wParam, LPARAM lParam );
	INT	 OnSysCommand( INT Command );
	LONG WndProc( UINT Message, UINT wParam, LONG lParam ) { return WWindow::WndProc( Message, wParam, lParam ); }
	WDlgObject* ObjectViewDlg;
	WDebugEdit Edit;
	WListView Watch;
	HWND hWndToolBar;
	WToolTip* ToolTipCtrl;
	WTreeView TreeView;
	WComboBox StackCombo;	
	WEdit	HideTypes;
	WCheckBox HideLocalVars;
	WCheckBox HideInstanceVars;
	WCheckBox HideActorVars;
	WButton   ObjectView;
	HWND hWndStatusBar;
	HWND hWndStaticBox;
	void SnoopKeyDown( WWindow* Src, INT Char );
	void OnHideTypesChange();
	void OnContextChange();
	void OnHideButtonsClick();
	void OnWatchRightClick();
	void OnTreeViewSelChanged();
	void OnTreeViewItemExpanding();
	void OnTreeViewDblClk();
	void OnEditRightClick();
	void AddChildren( const TCHAR* pParentName, HTREEITEM hti );
};

// Set variable value popup dialog
class WDlgOptions : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgOptions,WDialog,UnrealEd)

	UDebuggerWindow* Parent;

	// Constructor.
	WDlgOptions( UDebuggerWindow* InContext, WWindow* InOwnerWindow )
	:	WDialog			( TEXT("Debugger Options"), IDDIALOG_Options, InOwnerWindow )
	{
		Parent = InContext;
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgSetVar::OnInitDialog);
		WDialog::OnInitDialog();
		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgSetVar::OnDestroy);
		EndDialog(FALSE);
		WDialog::OnDestroy();
		unguard;
	}
	virtual INT DoModal()
	{
		guard(WDlgSetVar::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}

};

// Set variable value popup dialog
class WDlgObject : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgObject,WDialog,UnrealEd)

	UDebuggerWindow* Parent;
	WListView VarList;
	WEdit	  ObjName;
	WButton	  Refresh;
	INT PropCtr;
	TArray<UProperty*> Properties;
	TArray<void*> PropertyData;
	
	// Constructor.
	WDlgObject( UDebuggerWindow* InContext, WWindow* InOwnerWindow )
	:	WDialog			( TEXT("Debugger Options"), IDD_WHATEVER, InOwnerWindow )
	,	Refresh		( this, IDC_REFRESH,			FDelegate(this,(TDelegate)OnRefresh) )
	,	ObjName	    ( this, IDC_OBJNAME )
	,	VarList		( this, IDC_VARLIST )
	{
		Parent = InContext;
	}

	void OnRefresh()
	{
		FString ObjectName = *ObjName.GetText();
		UObject* FoundObject = NULL;
		for( TObjectIterator<UObject> It; It; ++It )
		{
			if ( ObjectName == It->GetName() )
			{
				FoundObject = *It;
				break;
			}
		}
		// FindObject<UObject>( NULL, *ObjName.GetText() );
		if ( FoundObject )
		{
			Properties.Empty();
			ListView_DeleteAllItems( VarList.hWnd );
			PropCtr = 0;
			for(TFieldIterator<UProperty> PropertyIt(FoundObject->GetClass());PropertyIt;++PropertyIt)
			{
				UProperty* Prop = *PropertyIt;

				TCHAR Buffer[256];
				LVITEMA Item1;

				if ( Prop->ArrayDim != 1 )
					return;
				
				Item1.mask = LVIF_TEXT | LVIF_STATE;
				Item1.state = 0;
				Item1.stateMask = 0;	
				Item1.iSubItem = 0;
				FString PropName;
				
				// Add property to the loaded properties array, so we can access each property
				// later by index in the ListView;
				Properties.Add();
				Properties(PropCtr) = Prop;

				PropName = FString::Printf(TEXT("Var %s"), Prop->GetName() );
				
				appStrcpy( Buffer, *PropName );
				
				Item1.pszText = TCHAR_TO_ANSI(Buffer);
				Item1.iItem = PropCtr;
				
				SendMessageX( VarList.hWnd, LVM_INSERTITEMA, 0, (LPARAM)(const LPLVITEM)&Item1 );
				
					
				void* PropAddr = NULL;

				PropAddr = (BYTE*)FoundObject + Prop->Offset;
				
				PropertyData.Add();
				PropertyData(PropCtr) = PropAddr;
				
				FString Result = Parent->GetPropText( Prop, PropAddr );

				appStrcpy( Buffer, *Result );

				ListView_SetItemTextA( VarList.hWnd, PropCtr, 1, TCHAR_TO_ANSI(Buffer) );
				
				PropCtr++;
			}
		}
	}


	void OnClose()
	{
		Show(0);
		ListView_DeleteAllItems( VarList.hWnd );
		Properties.Empty();
		PropertyData.Empty();
		PropCtr = 0;
	}

	// WDialog interface.
	void OnDestroy()
	{
		guard(WDlgBrushBuilder::OnDestroy);
		
		WDialog::OnDestroy();

		::DestroyWindow( hWnd );
		
		unguard;
	}

	virtual void DoModeless()
	{
		guard(DoModeless::DoModeless);
		_Windows.AddItem( this );
		hWnd = CreateDialogParamA( hInstance, MAKEINTRESOURCEA(IDD_WHATEVER), OwnerWindow->hWnd, (DLGPROC)StaticDlgProc, (LPARAM)this );
		if( !hWnd )
			appGetLastError();
		LVCOLUMNA lvcol;
		lvcol.mask = LVCF_TEXT | LVCF_WIDTH;
		lvcol.pszText = "Variable Name";
		lvcol.cx = 175;
		
		ListView_InsertColumnA( VarList.hWnd, 0, &lvcol );
		
		LVCOLUMNA lvcol2;
		lvcol2.mask = LVCF_TEXT | LVCF_WIDTH;
		lvcol2.pszText = "Variable Value";
		lvcol2.cx = 180;

		ListView_InsertColumnA( VarList.hWnd, 1, &lvcol2 );
		VarList.DblClkDelegate = FDelegate( this, (TDelegate)OnListDblClick );
		Show(1);
		unguard;
	}



	void OnListDblClick()
	{
		LVHITTESTINFO HitTest;
		HitTest.pt.x = 0;
		POINT ptScreen;
		::GetCursorPos( &ptScreen );
		HitTest.pt = ptScreen;
		::ScreenToClient( VarList.hWnd, &HitTest.pt );

		int item = ListView_HitTest( VarList.hWnd, &HitTest );
		if ( HitTest.flags | LVHT_ONITEMICON | LVHT_ONITEMLABEL | LVHT_ONITEMSTATEICON )
		{	
			if ( Properties(item)->IsA( UObjectProperty::StaticClass() ) )
			{
				FString ObjNameN = Parent->GetPropText( Properties(item), PropertyData(item) );
				ObjName.SetText( *ObjNameN );
				OnRefresh();
			}
		}
	}
};


// Set variable value popup dialog
class WDlgSetVar : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgSetVar,WDialog,UnrealEd)

	FString ReturnValue;

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WEdit NewName;

	UDebuggerWindow* Parent;
	UProperty* EditProp;
	
	// Constructor.
	WDlgSetVar( UDebuggerWindow* InContext, WWindow* InOwnerWindow )
	:	WDialog			( TEXT("Set variable value"), IDDIALOG_SET_VAR, InOwnerWindow )
	,	OkButton		( this, IDOK,			FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton	( this, IDCANCEL,		FDelegate(this,(TDelegate)OnCancel) )
	,	NewName		    ( this, IDC_NEWNAME )
	{
		Parent = InContext;
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgSetVar::OnInitDialog);
		WDialog::OnInitDialog();
		NewName.SetText( TEXT("") );
		::SetFocus( NewName.hWnd );
		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgSetVar::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	virtual INT DoModal()
	{
		guard(WDlgSetVar::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		ReturnValue = NewName.GetText();
		EndDialog(TRUE);
	}
	void OnCancel()
	{
		guard(WDlgSetVar::OnCancel);
		EndDialog(FALSE);
		unguard;
	}

};

// A breakpoint
class FBreakpoint
{
public:
	FBreakpoint( FString _ClassName, INT _Line )
	{
		ClassName = _ClassName;
		Line = _Line;
		IsEnabled = 1;
	}

	
	FBreakpoint& operator=( const FBreakpoint& Other )
	{
		ClassName = Other.ClassName;
		Line = Other.Line;
		IsEnabled = Other.IsEnabled;
		return *this;
	}


	FString ClassName;
	INT Line;
	UBOOL IsEnabled;
};

class FBreakpointManager
{
public:
	UBOOL QueryBreakpoint( FString ClassName, INT Line );
	void SetBreakpoint( FString ClassName, INT Line );
	void RemoveBreakpoint( FString ClassName, INT Line );
	void ToggleBreakpoint( FString ClassName, INT Line );
	void Serialize( FArchive& Ar );
private:

	TArray<FBreakpoint> Breakpoints;
};
