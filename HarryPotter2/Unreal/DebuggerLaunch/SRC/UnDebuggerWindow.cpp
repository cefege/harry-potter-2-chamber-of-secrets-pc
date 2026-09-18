/*=============================================================================
	UnDebuggerWindow.cpp: Debugger Windows code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Lucas Alonso.
=============================================================================*/

// All this code is in need of a major cleanup, since there's alot of stuff
// leftover from the various partial rewrites it underwent.

#include "DebuggerLaunchPrivate.h"

UDebuggerWindow::UDebuggerWindow()
: 	WWindow( TEXT("UDebugger"), NULL )
,	Edit			 ( this )
,	Watch			 ( this )
,   HideLocalVars    ( this )
,   HideInstanceVars ( this )
,   HideActorVars    ( this )
,   HideTypes		 ( this )
,	TreeView		 ( this )
,	StackCombo		 ( this )
{
	debugf(NAME_Init, TEXT("UDebuggerWindow created."));
	CurrentState = NULL;
	PendingState = NULL;
	DSIdleState* InitialState = new DSIdleState;
	ChangeState( InitialState );
	BreakpointManager = new FBreakpointManager;
	CallStack = new FCallStack( this );
	WatchNode = NULL;
	OpenWindow();
	IsDebugging = 0;
	NeedsClear = 0;
	bClosing = 0;
	ObjectViewDlg = NULL;
	bShowOnlyActorClasses = 1;
	Show(1);
	SendMessageX( Edit.hWnd, EM_SETTARGETDEVICE, (WPARAM)NULL, (LPARAM)1 );

	// Load up the initial breakpoint set, if available
	FArchive* BReader = GFileManager->CreateFileReader( TEXT("Startup.udbg") );
	if( BReader )
	{
		BreakpointManager->Serialize( *BReader );
		BReader->Close();
	}
}


void UDebuggerWindow::OnWatchRightClick()
{
	LVHITTESTINFO HitTest;
	HitTest.pt.x = 0;
	POINT ptScreen;
	::GetCursorPos( &ptScreen );
	HitTest.pt = ptScreen;
	::ScreenToClient( Watch.hWnd, &HitTest.pt );

	int item = ListView_HitTest( Watch.hWnd, &HitTest );
	if ( HitTest.flags | LVHT_ONITEMICON | LVHT_ONITEMLABEL | LVHT_ONITEMSTATEICON )
	{
		//MessageBox( NULL, TEXT("Please be extremely careful! :)") , FString::Prinftf(TEXT("New value for variable '%s':"), LoadedProperties(item)->GetName())), MB_OK );
		WDlgSetVar SetVarDlg( this, this );
		if ( SetVarDlg.DoModal() )
		{
			FString NewValue = SetVarDlg.ReturnValue;
			LoadedProperties(item)->ImportText( *NewValue, (BYTE*)LoadedPropertyData(item), 0 );
			if ( CallStack->GetStackDepth() > 0 && WatchNode )
			{
				RefreshWatch( WatchNode );
			}
		}
	}

}

void UDebuggerWindow::RefreshRichText( UClass* Class )
{
	FString FullName( Class->GetFullName() );
	FString RText = ScriptHash.FindRef( *FullName );
	FStringOutputDevice GetPropResult = FStringOutputDevice();
	FormatRichText( Class, *RText, GetPropResult );
	FString Temp = *GetPropResult;
	HighlightHash.Set( *FullName, *Temp );
}

// Loads a Class's script out of the corresponding UC file, since we can't
// access Class->ScriptText during gameplay.
UBOOL UDebuggerWindow::LoadClassText( UClass* Class )
{
	guard(UDebuggerWindow::LoadClassText);
	FString FullName( Class->GetFullName() );

	if ( ScriptHash.FindRef( FullName ).Len() == 0  )
	{
		// GetFullName returns a name in the form of:
		// Class [Package].[ClassName]
		int CutPos = FullName.InStr( TEXT(".") );

		// Extract the package name and chop off the 'Class' thing.
		FString PackageName = FullName.Left( CutPos );
		PackageName = PackageName.Right( PackageName.Len() - 6 );

		// And finally, grab the classname, and construct the file
		// location.
		FString ClassName = FullName.Right( FullName.Len() - CutPos - 1 );
		FString ScriptFile = FString::Printf( TEXT("..\\%s\\Classes\\%s.uc"), *PackageName, *ClassName );
		FString Temp;

		GLog->Logf(TEXT("Loading %s..."), *ScriptFile );
		if ( appLoadFileToString( Temp, *ScriptFile, GFileManager ) )
		{
		}
		else
		{
			//Make sure it's not in one of the sub directories   ft
			FString Spec = FString(TEXT("..")) * PackageName * TEXT("Classes\\*");
			FString FileName;
			TArray<FString> Directories = GFileManager->FindFiles( *Spec, 0, 1 );
			for( INT i = 0; i < Directories.Num(); i++ )
			{
				FileName = FString(TEXT("..")) * PackageName * TEXT("Classes") * Directories(i) * ClassName + TEXT(".uc");
				//FileTime = GFileManager->GetGlobalTime( *FileName );
				//if( FileTime > 0 )
				if( appLoadFileToString( Temp, *FileName, GFileManager ) )
					break;
			}

			if( Temp.Len() <= 0 )
			{
				appThrowf(TEXT("Error! Could not load file."));
				return FALSE;
			}
		}

		ScriptHash.Set( *FullName, *Temp );
		RefreshRichText( Class );

		SetStatus( *FString::Printf(TEXT("Loaded script %s"), *ScriptFile) );

	}
	return TRUE;
	unguard;
}

// Extract the value of a given property at a given address
// Now correctly handles Struct properties by recursing.
FString UDebuggerWindow::GetPropText( UProperty* Prop, void* PropAddr )
{
	FString Result;

	// This SHOULD be sufficient.
	BYTE* ResultBuffer = new BYTE[1024];
	appMemzero( ResultBuffer, 1024 );

	Prop->CopyCompleteValue( ResultBuffer, PropAddr );

	if ( Prop->IsA( UStructProperty::StaticClass() ) )
	{
		Result = TEXT("(");
		// Recurse every property in this struct, and copy it's value into Result;
		for( TFieldIterator<UProperty> It(Cast<UStructProperty>(Prop)->Struct); It; ++It )
			Result += FString::Printf(TEXT("%s=%s, "), It->GetName(), GetPropText( *It, (BYTE*)PropAddr + It->Offset ) );
		Result = Result.Left(Result.Len()-2);
		Result += TEXT(")");
	}
	else if ( Prop->IsA( UIntProperty::StaticClass() ) )
	{
		INT IntResult = *(INT*)ResultBuffer;
		Result = FString::Printf(TEXT("%i"), IntResult );
	}
	else if ( Prop->IsA( UByteProperty::StaticClass() ) )
	{
		// Give it to printf as an integer.
		INT ByteResult = *(BYTE*)ResultBuffer;
		Result = FString::Printf(TEXT("%i"), ByteResult );
	}
	else if ( Prop->IsA( UFloatProperty::StaticClass() ) )
	{
		FLOAT FloatResult = *(FLOAT*)ResultBuffer;
		Result = FString::Printf(TEXT("%f"), FloatResult );
	}
	else if ( Prop->IsA( UClassProperty::StaticClass() ) )
	{
		UClass* ClassResult = *(UClass**)ResultBuffer;
		if ( ClassResult != NULL )
			Result = FString::Printf(TEXT("Class'%s'"), ClassResult->GetName() );
		else
			Result = TEXT("None");
	}
	else if ( Prop->IsA( UObjectProperty::StaticClass() ) )
	{
		UObject* ObjResult = *(UObject**)ResultBuffer;
		if ( ObjResult != NULL )
			Result = FString::Printf(TEXT("%s"), ObjResult->GetName() );
		else
			Result = TEXT("None");
	}
	else if ( Prop->IsA( UNameProperty::StaticClass() ) )
	{
		FName NameResult = *(FName*)ResultBuffer;
		Result = FString::Printf(TEXT("'%s'"), *NameResult);
	}
	else if ( Prop->IsA( UStrProperty::StaticClass() ) )
	{
		FString StringResult = *(FString*)ResultBuffer;
		Result = FString::Printf(TEXT("\"%s\""), *StringResult );
	}
	else if ( Prop->IsA( UBoolProperty::StaticClass() ) )
	{
		INT BoolResult = *(BITFIELD*)ResultBuffer;
		Result = BoolResult ? TEXT("True") : TEXT("False");
	}
	else
		Result = TEXT("Handler Not Yet Implemented");

	// Clean up our buffer
	delete ResultBuffer;

	return Result;
}

void ParseIntoArray( FString In, const TCHAR* pchDelim, TArray<FString>* InArray)
{
	guard(FString::ParseIntoArray);
	check(InArray);

	FString S = In;

	for( INT i = S.InStr( pchDelim ) ; i > 0 ; )
	{
		new(*InArray)FString( S.Left(i) );
		S = S.Mid( i + 1, S.Len() );
		i = S.InStr( pchDelim );
	}

	new(*InArray)FString( S );
	unguard;
}


void Split( FString In, INT& R, INT& G, INT& B )
{
	TArray<FString> Components;
	ParseIntoArray( In, TEXT(","), &Components );

	R = appAtoi( *Components(0) );
	G = appAtoi( *Components(1) );
	B = appAtoi( *Components(2) );
}



void UDebuggerWindow::LoadIni()
{
	if( GFileManager->FileSize(TEXT("udebugger.ini"))<0 )
	{
		FString Text;
		Text = TEXT("[UDebugger.ColorConfig]\n");
		Text = Text + TEXT("BackColor=255,255,255\n");
		Text = Text + TEXT("TextColor=0,0,0\n");
		Text = Text + TEXT("CommentColor=0,128,0\n");
		Text = Text + TEXT("KeywordColor=0,0,255\n");
		Text = Text + TEXT("LabelColor=255,255,0\n");
		Text = Text + TEXT("ExecColor=128,128,128\n");
		Text = Text + TEXT("BreakpointFrontColor=255,255,255\n");
		Text = Text + TEXT("BreakpointBackColor=255,0,0\n");
		Text = Text + TEXT("StringConstantColor=0,128,0\n");
		Text = Text + TEXT("NameConstantColor=255,0,255\n");
		appSaveStringToFile( Text, TEXT("udebugger.ini") );

	}



	#define INSERT_COLOR_RTF(t) \
	INT R##t; \
	INT G##t; \
	INT B##t; \
	Split( t, R##t, G##t, B##t ); \
	ColorConfig += FString::Printf(TEXT("\\red%i\\green%i\\blue%i;"), R##t, G##t, B##t ); \

	TMultiMap<FString,FString>* Sec = GConfig->GetSectionPrivate( TEXT("UDebugger.ColorConfig"), 0, 1, TEXT("udebugger.ini") );

	FString BackColor = *Sec->Find( TEXT("BackColor") );
	FString TextColor = *Sec->Find( TEXT("TextColor") );
	FString CommentColor = *Sec->Find( TEXT("CommentColor") );
	FString KeywordColor = *Sec->Find( TEXT("KeywordColor") );
	FString LabelColor = *Sec->Find( TEXT("LabelColor") );
	FString ExecColor = *Sec->Find( TEXT("ExecColor") );
	FString BreakpointFrontColor = *Sec->Find( TEXT("BreakpointFrontColor") );
	FString BreakpointBackColor = *Sec->Find( TEXT("BreakpointBackColor") );
	FString StringConstantColor = *Sec->Find( TEXT("StringConstantColor") );
	FString NameConstantColor = *Sec->Find( TEXT("NameConstantColor") );


	ColorConfig = TEXT("{\\colortbl");

	INT bR;
	INT bG;
	INT bB;
	Split( BackColor, bR, bG, bB );

	SendMessageX( Edit.hWnd, EM_SETBKGNDCOLOR, 0, (LPARAM)(COLORREF)RGB(bR,bG,bB) );

	INSERT_COLOR_RTF(TextColor);
	INSERT_COLOR_RTF(CommentColor);
	INSERT_COLOR_RTF(KeywordColor);
	INSERT_COLOR_RTF(LabelColor);
	INSERT_COLOR_RTF(ExecColor);
	INSERT_COLOR_RTF(BreakpointFrontColor);
	INSERT_COLOR_RTF(BreakpointBackColor);
	INSERT_COLOR_RTF(StringConstantColor);
	INSERT_COLOR_RTF(NameConstantColor);

	ColorConfig = ColorConfig + TEXT("}\r\n");
	GLog->Logf(*ColorConfig);
}

void UDebuggerWindow::LoadEditPackages()
{
	TArray<FString> EditPackages;
	TMultiMap<FString,FString>* Sec = GConfig->GetSectionPrivate( TEXT("Editor.EditorEngine"), 0, 1, 0 );
	if(Sec==0)		//gk
		return;
	Sec->MultiFind( FString(TEXT("EditPackages")), EditPackages );
	TObjectIterator<UEngine> EngineIt;
	if ( EngineIt )
		for( INT i=0; i<EditPackages.Num(); i++ )
			if( !EngineIt->LoadPackage( NULL, *EditPackages(i), LOAD_NoWarn ) )
				appErrorf( TEXT("Can't find edit package '%s'"), *EditPackages(i) );


	RebuildTree();
}

// Insert a given property into the watch window.
void UDebuggerWindow::InsertProperty( UProperty* Prop, UObject* Obj, FFrame* Stack, UBOOL bIsLocal )
{
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
	LoadedProperties.Add();
	LoadedProperties(ItemCtr) = Prop;

	if ( bIsLocal )
		PropName = FString::Printf(TEXT("Local %s"), Prop->GetName() );
	else
		PropName = FString::Printf(TEXT("Var %s"), Prop->GetName() );

	appStrcpy( Buffer, *PropName );

	Item1.pszText = TCHAR_TO_ANSI(Buffer);
	Item1.iItem = ItemCtr;

	ListView_InsertItemA( Watch.hWnd, &Item1 );

	void* PropAddr = NULL;

	if ( bIsLocal )
		PropAddr = Stack->Locals + Prop->Offset;
	else
		PropAddr = (BYTE*)Obj + Prop->Offset;

	LoadedPropertyData.Add();
	LoadedPropertyData(ItemCtr) = PropAddr;

	FString Result = GetPropText( Prop, PropAddr );

	appStrcpy( Buffer, *Result );

	ListView_SetItemTextA( Watch.hWnd, ItemCtr, 1, TCHAR_TO_ANSI(Buffer) );

	if ( bIsLocal )
		appStrcpy( Buffer, *FString::Printf(TEXT("Function %s"), Stack->Node->GetName()) );
	else
		appStrcpy( Buffer, *FString::Printf(TEXT("Class %s"), Prop->GetOuter()->GetName()) );

	ListView_SetItemTextA( Watch.hWnd, ItemCtr, 2, TCHAR_TO_ANSI(Buffer) );


	ItemCtr++;
}

// Update the Watch ListView with all the current variables the Stack/Object contain.
void UDebuggerWindow::RefreshWatch( FStackNode* CNode )
{

	ListView_DeleteAllItems( Watch.hWnd );
	ItemCtr = 0;
	if ( CNode == NULL )
		return;
	UFunction* Function = Cast<UFunction>(CNode->StackNode->Node);
	UProperty*	ReturnValue = FindField<UProperty>(Function,TEXT("ReturnValue"));
	UProperty* Parm;
	int Index = 0;
	// Add local vars, if the user hasn't requested otherwise;
	if ( Function != NULL )
	{
		if ( !HideLocalVars.IsChecked() )
		{
			for(Parm = Function->PropertyLink,Index = 0;Parm && Index < Function->NumParms;Parm = Parm->PropertyLinkNext,Index++)
			{
				if(Parm == ReturnValue)
				{
					Parm = Parm->PropertyLinkNext;
					break;
				}
				InsertProperty( Parm, CNode->Object, CNode->StackNode, TRUE );
			}

			if(Function->Script.Num())
			{
				for(;Parm;Parm = Parm->PropertyLinkNext)
				{
					InsertProperty( Parm, CNode->Object, CNode->StackNode, TRUE );
				}
			}
		}
	}
	if ( HideActorVars.IsChecked() && !HideInstanceVars.IsChecked() )
	{
		FString SkipClasses[20];
		int		SkipCtr = 0;
		FString SkipClassesRaw = HideTypes.GetText();

		const TCHAR* ch = *SkipClassesRaw;
		// Parse a string in the format: "AClassName,AnotherClassName,ClassName3"
		FString CurrentToken;
		for(int i=0;i<SkipClassesRaw.Len();i++)
		{
			if ( *ch == *TEXT(",") )
			{
				SkipClasses[SkipCtr++] = CurrentToken;
				CurrentToken = TEXT("");
				ch++;
				continue;
			}
			TCHAR OneChar[2];
			OneChar[0] = *ch;
			OneChar[1] = 0;
			CurrentToken += OneChar;
			ch++;
		}
		// Add last token onto the stack (this way we avoid the need for a trailing comma)
		SkipClasses[SkipCtr++] = CurrentToken;

		// Now iterate through each UProperty this Object has, and makes sure it's outer
		// is not one of the classes that the user wants to hide.
		for(TFieldIterator<UProperty> PropertyIt(CNode->Object->GetClass());PropertyIt;++PropertyIt)
		{
			UBOOL bSkip = 0;
			FString PropClass = PropertyIt->GetOuter()->GetName();
			for( int j=0;j<SkipCtr;j++ )
				if ( SkipClasses[j] == PropClass )
				{
					bSkip = 1;
					break;
				}

			// No? Then insert the property into the window.
			if ( !bSkip )
				InsertProperty( *PropertyIt, CNode->Object, CNode->StackNode, FALSE );
		}
	}
	else if ( !HideActorVars.IsChecked() && !HideInstanceVars.IsChecked() )
	{
		// Insert 'em all.
		for(TFieldIterator<UProperty> PropertyIt(CNode->Object->GetClass());PropertyIt;++PropertyIt)
			InsertProperty( *PropertyIt, CNode->Object, CNode->StackNode, FALSE );
	}
}

// One of the three buttons was clicked, so we refresh the watch list
void UDebuggerWindow::OnHideButtonsClick()
{
	if ( HideActorVars.IsChecked() )
		EnableWindow( HideTypes.hWnd, TRUE );
	else
		EnableWindow( HideTypes.hWnd, FALSE );
	if ( CallStack->GetStackDepth() > 0 && WatchNode )
	{
		RefreshWatch( WatchNode );
	}
}

void UDebuggerWindow::OnHideTypesChange()
{
	OnHideButtonsClick();
}

void UDebuggerWindow::SnoopKeyDown( WWindow* Src, INT Char )
{
	OnKeyUp( Char, 0 );
}


void UDebuggerWindow::OnKeyUp( WPARAM wParam, LPARAM lParam )
{
	// A hack to get the familiar hotkeys working again.  This should really go through
	// Proper Windows accelerators, but I can't get them to work.
	switch( wParam )
	{
		case VK_F5:
			OnCommand( ID_DEBUG_GO );
			break;
		case VK_F9:
			OnCommand( ID_DEBUG_STEPOVERFUNCTIONCALL );
			break;
		case VK_F11:
			OnCommand( ID_DEBUG_STEPINTO );
			break;
		case VK_F12:
			OnCommand( ID_DEBUG_STEPOUT );
			break;

	}
}

// Menu item was clicked
void UDebuggerWindow::OnCommand( INT Command )
{
	INT CharPos, Line, CurTop;
			POINTL P;
		TVHITTESTINFO tvhti;
	switch( Command )
	{
	case ID_DEBUGPOPUP_INSERTBREAKPOINT:
		tvhti.pt = ContextClick;
		::ScreenToClient( Edit.hWnd, &tvhti.pt );
		P.x = tvhti.pt.x;
		P.y = tvhti.pt.y;
		CharPos = (DWORD)SendMessageX( Edit.hWnd, EM_CHARFROMPOS, 0L, (LPARAM)&P );
		Line = SendMessageX( Edit.hWnd, EM_EXLINEFROMCHAR, 0L, (LPARAM)CharPos,  );
		BreakpointManager->SetBreakpoint( CurrentLoadedClass->GetName(), Line );
		CurTop = SendMessageX(Edit.hWnd, EM_GETFIRSTVISIBLELINE, 0, 0);
		RefreshRichText( CurrentLoadedClass );
		SetClass( CurrentLoadedClass, TRUE );
		ScrollToLine( CurTop );
		SetStatus( *FString::Printf(TEXT("Set breakpoint on line %i."), Line) );
		break;
	case ID_DEBUGPOPUP_REMOVEBREAKPOINT:

		tvhti.pt = ContextClick;
		::ScreenToClient( Edit.hWnd, &tvhti.pt );
		P.x = tvhti.pt.x;
		P.y = tvhti.pt.y;
		CharPos = (DWORD)SendMessageX( Edit.hWnd, EM_CHARFROMPOS, 0L, (LPARAM)&P );
		Line = SendMessageX( Edit.hWnd, EM_EXLINEFROMCHAR, 0L, (LPARAM)CharPos,  );
		BreakpointManager->RemoveBreakpoint( CurrentLoadedClass->GetName(), Line );
		CurTop = SendMessageX(Edit.hWnd, EM_GETFIRSTVISIBLELINE, 0, 0);
		RefreshRichText( CurrentLoadedClass );
		SetClass( CurrentLoadedClass, TRUE );
		ScrollToLine( CurTop );
		SetStatus( *FString::Printf(TEXT("Removed breakpoint on line %i."), Line) );
		break;

	case ID_DEBUG_RUNTOCURSOR:
		CurrentState->HandleInput( UA_RunToCursor );
		break;
	case ID_TOOLS_OPTIONS:
	{
		WDlgOptions Options( this, this );
		if ( Options.DoModal() )
		{

		}
		SetFocus( hWnd );
		break;
	}
	case ID_TOOLS_OBJECTVIEW:
	{
		if ( !ObjectViewDlg )
		{
			ObjectViewDlg = new WDlgObject( this, this );
			ObjectViewDlg->DoModeless();
		}
		else
		{
			ObjectViewDlg->Show(1);
		}
	}
	case ID_TOOLS_REFRESHWATCH:
		if ( CallStack->GetStackDepth() > 0 && WatchNode )
			RefreshWatch( WatchNode );
		break;
	case ID_TOOLS_RELOADEDITPACKAGES:
		LoadEditPackages();
		break;
	case ID_TOOLS_REFRESHCLASSTREE:
		RebuildTree();
		break;
	case ID_TOOLS_TOGGLESHOWOBJECTACTORCLASSES:
		bShowOnlyActorClasses = !bShowOnlyActorClasses;
		if ( bShowOnlyActorClasses )
			CheckMenuItem(GetMenu(hWnd), ID_TOOLS_TOGGLESHOWOBJECTACTORCLASSES, MF_CHECKED);
		else
			CheckMenuItem(GetMenu(hWnd), ID_TOOLS_TOGGLESHOWOBJECTACTORCLASSES, MF_UNCHECKED);
		RebuildTree();
		break;
	case ID_DEBUG_GO:
		CurrentState->HandleInput( UA_Go );
		break;
	case ID_DEBUG_BREAKEXECUTION:
		appMsgf(TEXT("Not yet implemented!"));
		break;
	case ID_DEBUG_STOPDEBUGGING:
	case ID_FILE_EXIT:
		GIsRequestingExit = 1;
	case ID_DEBUG_STEPINTO:
		CurrentState->HandleInput( UA_StepInto );
		break;
	case ID_DEBUG_STEPOVER:
		CurrentState->HandleInput( UA_StepOver );
		break;
	case ID_DEBUG_STEPOUT:
		CurrentState->HandleInput( UA_StepOut );
		break;
	case ID_DEBUG_STEPOVERFUNCTIONCALL:
		CurrentState->HandleInput( UA_StepOverStack );
		break;
	case ID_FILE_SAVEAS:
		SaveBreakpoints();
		break;
	case ID_FILE_LOAD:
		LoadBreakpoints();
		break;
	}
}

void UDebuggerWindow::CloseObjectWindow()
{
	delete ObjectViewDlg;
	ObjectViewDlg = 0;
}

void UDebuggerWindow::ScrollToLine( INT Line, INT bCenter )
{
	LockWindowUpdate( hWnd );
	//Line -= 10;
	if ( bCenter )
	{
		FRect Rect = Edit.GetClientRect();
		INT Height = Rect.Height();
		INT LineSize = 8 + 6;
		INT Lines = Height = LineSize;
		Line -= Lines / 2;	// Center
	}
	INT CurTop = SendMessageX(Edit.hWnd, EM_GETFIRSTVISIBLELINE, 0, 0);
	while( CurTop > Line )
	{
		SendMessageX(Edit.hWnd, EM_SCROLL, SB_LINEUP, 0);
		CurTop--;
	}
	while( CurTop < Line )
	{
		SendMessageX(Edit.hWnd, EM_SCROLL, SB_LINEDOWN, 0);
		CurTop++;
	}
	LockWindowUpdate( NULL );

}

int UDebuggerWindow::FindPos( const TCHAR* InText, int Line )
{
	char ch10 = '\x0a', *pch = TCHAR_TO_ANSI( InText );
	INT iChar = 0, iLine = 1;

	while( *pch && iLine < Line )
	{
		if( *pch == ch10 )
			iLine++;

		iChar++;
		pch++;
	}
	return iChar;
}

UBOOL UDebuggerWindow::Parse_SkipWhiteSpace( char*& pch, int& iChar, UBOOL bReverse )
{
	char space = '\x20';
	char tab = '\x09';
	if ( bReverse )
	{
		while(1)
		{
			if( *pch != space && *pch != tab )
			{
				pch--;
				iChar--;
				break;
			}
			iChar--;
			pch--;
		}
	}
	else
	{
		while(1)
		{
			if( *pch != space && *pch != tab )
			{
				break;
			}
			iChar++;
			pch++;
		}
	}
	return TRUE;
}

UBOOL UDebuggerWindow::Parse_MatchChar( char*& pch, int& iChar, char Match, UBOOL bReverse )
{
	if ( bReverse )
	{
		while(1)
		{
			if( *pch == Match )
			{
				break;
			}
			iChar--;
			pch--;
		}
	}
	else
	{
		while(1)
		{
			if( *pch == Match )
			{
				break;
			}
			iChar++;
			pch++;
		}
	}
	return TRUE;
}

UBOOL UDebuggerWindow:: Parse_MatchParenBlocks( char*& pch, int& iChar, UBOOL bReverse, int InitialBalance )
{
	char lparen = '(';
	char rparen = ')';

	if ( bReverse )
	{
		int Balance = InitialBalance;
		while(1)
		{
			if( *pch == rparen )
			{
					Balance++;
			}
			if ( *pch == lparen )
			{
					Balance--;
					if ( Balance == 0 )
					{
						break;
					}
			}
			iChar--;
			pch--;
		}
	}
	else
	{
		int Balance = InitialBalance;
		while(1)
		{
			if( *pch == lparen )
			{
					Balance++;
			}
			if ( *pch == rparen )
			{
					Balance--;
					if ( Balance == 0 )
					{
						break;
					}
			}
			iChar++;
			pch++;
		}
	}
	return TRUE;
}

FString UDebuggerWindow::Parse_MatchToken( char*& pch, int& iChar, UBOOL bReverse )
{
	char Buff[128];
	char Buff2[128];
	int ptr = 0;
	if ( bReverse )
	{
		while(1)
		{
			char c = *pch;
			if( !( (c>='a' && c<='z')
				|| (c>='A' && c<='Z')
				|| (c>='0' && c<='9')
				|| (c==']' || c=='[') )
			  )
			{
				break;
			}
			Buff[ptr++] = c;
			iChar--;
			pch--;
		}

		for( int i=ptr-1; i>=0; i-- )
		{
			Buff2[i] = Buff[ptr-i-1];
		}
		iChar++;
		pch++;
		return ANSI_TO_TCHAR(Buff2);
	}
	else
	{
		while(1)
		{
			char c = *pch;
			if( !((c>='a' && c<='z') || (c>='A' && c<='Z') || (c>='0' && c<='9')) )
			{
				Buff[ptr++] = c;
				break;
			}
			else

			iChar--;
			pch--;
		}
		return ANSI_TO_TCHAR(Buff);
	}
}


void UDebuggerWindow::SetDebuggerLine( FStackNode* CNode )
{
	if ( CNode == NULL )
	{
		SendMessageX( Edit.hWnd, EM_SETSEL, 0, 0 );
		SendMessageX( Edit.hWnd, EM_SCROLLCARET, 0, 0 );
		return;
	}

	int CompilerPos = CNode->Pos;

	// Figure out where the current line begins and ends, so
	// we can have the richtext box select it.
	int StartOfLine = FindPos( *(ScriptHash.FindRef( CurrentLoadedClass->GetFullName() )), CNode->Line );
	int EndOfLine = FindPos( *(ScriptHash.FindRef( CurrentLoadedClass->GetFullName() )), CNode->Line+1 ) - 1;

	int StartSelectionPos  = CompilerPos;
	int EndSelectionPos    = CompilerPos;


	#define MOVE_TO(x) \
	iChar = x; \
	pch = ch + x;

	#define NEXTCHAR() \
	pch++;		\
	iChar++;	\

	#define PREVCHAR() \
	pch--;	\
	iChar--;	\

	#define SET_STARTLINE() StartSelectionPos = StartOfLine;
	#define SET_ENDLINE() EndSelectionPos = EndOfLine;
	#define SET_STARTSEL() StartSelectionPos = iChar;
	#define SET_ENDSEL() EndSelectionPos = iChar;
	#define MATCH_PAREN_BLOCKS(b) Parse_MatchParenBlocks( pch, iChar, b );
	#define MATCH_PAREN_BLOCKS_IB(b,i) Parse_MatchParenBlocks( pch, iChar, b, i);
	#define SKIP_WHITESPACE(b) Parse_SkipWhiteSpace( pch, iChar, b );
	#define MATCH_CHAR(c,b) Parse_MatchChar( pch, iChar, c, b );
	#define MATCH_TOKEN(b) Parse_MatchToken( pch, iChar, b );
	char* ch = TCHAR_TO_ANSI( *(ScriptHash.FindRef( CurrentLoadedClass->GetFullName() )) );
	INT iChar = CompilerPos;
	char* pch = ch + CompilerPos;

	if ( CNode->Info == TEXT("SIMPLEIF") )
	{
		// if *( BooleanExpression )
		SKIP_WHITESPACE(TRUE);
		PREVCHAR();
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
		MATCH_PAREN_BLOCKS(FALSE);
		NEXTCHAR();
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("LET") )
	{
		// A = B*;

		SET_ENDSEL();
		MATCH_CHAR('=',TRUE);
		PREVCHAR();
		SKIP_WHITESPACE(TRUE);
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
	}
	else if ( CNode->Info == TEXT("WHILE") )
	{
		// while*( BooleanExpression )
		SET_STARTSEL();
		MATCH_PAREN_BLOCKS(FALSE);
		NEXTCHAR();
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("FORINIT") )
	{
		// for (* init; eval; inc )
		SKIP_WHITESPACE(FALSE);
		SET_STARTSEL();
		MATCH_CHAR(';', FALSE);
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("FOREVAL") )
	{
		// for ( init; *eval; inc )
		SKIP_WHITESPACE(FALSE);
		SET_STARTSEL();
		MATCH_CHAR(';', FALSE);
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("FORINC") )
	{
		// for ( init; eval; *inc )
		SKIP_WHITESPACE(FALSE);
		SET_STARTSEL();
		MATCH_PAREN_BLOCKS_IB(FALSE,1);
		SKIP_WHITESPACE(TRUE);
		SET_ENDSEL();
	}
	else if ( CNode->Info.InStr(TEXT("BREAK")) != -1 ||CNode->Info.InStr(TEXT("CONTINUE")) != -1)
	{
		// break*;
		// continue*;
		SET_ENDSEL();
		PREVCHAR();
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
	}
	else if ( CNode->Info == TEXT("ITERATOREFP") )
	{
		// foreach IteratorFunction( parameters )
		// *{
		MATCH_PAREN_BLOCKS(TRUE);
		SKIP_WHITESPACE(TRUE);
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
		MATCH_PAREN_BLOCKS(FALSE);
		NEXTCHAR();
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("EFP") )
	{
		// FunctionCall ( parameters )*
		PREVCHAR();
		MATCH_PAREN_BLOCKS(TRUE);
		SKIP_WHITESPACE(TRUE);
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
	}
	else if ( CNode->Info == TEXT("RETURN") )
	{
		// return *something;
		SKIP_WHITESPACE(TRUE);
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
		MATCH_CHAR(';', FALSE);
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("RETURNNOTHING") )
	{
		SET_STARTLINE();
		SET_ENDLINE();
	}
	else if ( CNode->Info == TEXT("SWITCH") )
	{
		// switch* ()
		SKIP_WHITESPACE(TRUE);
		MATCH_TOKEN(TRUE);
		SET_STARTSEL();
		MATCH_PAREN_BLOCKS(FALSE);
		NEXTCHAR();
		SET_ENDSEL();
	}
	else if ( CNode->Info == TEXT("ASSERT") )
	{
		// assert*()
		SET_STARTSEL();
		MATCH_PAREN_BLOCKS(FALSE);
		NEXTCHAR();
		SET_ENDSEL();
	}
	// Send the selection message.
	SendMessageX( Edit.hWnd, EM_SETSEL, StartSelectionPos, EndSelectionPos );
	SendMessageX( Edit.hWnd, EM_SCROLLCARET, 0, 0 );

	ScrollToLine( CNode->Line - 1, 1 );

	::SetFocus( Edit.hWnd );
}


void UDebuggerWindow::SetClass( UClass* Class, UBOOL bForce )
{
	if ( Class == NULL )
	{
		LockWindowUpdate(Edit.hWnd);
		char Blank[1];
		memset( &Blank, 0, sizeof(char)*2 );
		Edit.StreamTextIn( Blank, 1 );
		ScrollToLine( 0 );
		LockWindowUpdate(NULL);

		return;
	}
	if ( ( (CurrentLoadedClass != Class) || bForce ) && LoadClassText( Class ) )
	{
		const char* chScriptText = TCHAR_TO_ANSI(*(HighlightHash.FindRef( Class->GetFullName() )));
		// Stream it into the RichEdit control
		LockWindowUpdate(Edit.hWnd);
		Edit.StreamTextIn( (char*)chScriptText, strlen(chScriptText) );
		ScrollToLine( 0 );
		LockWindowUpdate(NULL);

	}
	CurrentLoadedClass = Class;
}

void UDebuggerWindow::OnDestroy()
{
	debugf(NAME_Init, TEXT("UDebuggerWindow shutdown."));
	WWindow::OnDestroy();
}

void UDebuggerWindow::Shutdown()
{
	ProcessPendingState();
	if ( CurrentState )
		delete CurrentState;
	if ( BreakpointManager )
		delete BreakpointManager;
	if ( CallStack )
		delete CallStack;

	CloseObjectWindow();

	CallStack = NULL;
	CurrentState = NULL;
	BreakpointManager = NULL;

	ScriptHash.Empty();
	HighlightHash.Empty();
	Show(0);
	debugf(NAME_Init, TEXT("UDebuggerWindow closed."));

	LoadedProperties.Empty();
	LoadedPropertyData.Empty();

	::DestroyWindow( hWndToolBar );
	delete ToolTipCtrl;
}

void UDebuggerWindow::OnSize( DWORD Flags, INT NewX, INT NewY )
{
	guard(UDebuggerWindow::OnSize);
	WWindow::OnSize( Flags, NewX, NewY );
	PositionChildControls();
	unguard;
}

// Show pop up context menu for the edit window.. used to insert/remove breakpoints
void UDebuggerWindow::ShowDebugContextMenu()
{
	POINT ptScreen;
	TVHITTESTINFO tvhti;

	::GetCursorPos( &ptScreen );

	tvhti.pt = ptScreen;
	::ScreenToClient( Edit.hWnd, &tvhti.pt );
	HMENU menu = GetSubMenu( LoadMenuIdX(hInstance, IDR_DEBUGCONTEXT), 0 );
	TrackPopupMenu( menu,
		TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
		ptScreen.x, ptScreen.y, 0,
		hWnd, NULL);

	ContextClick.x = ptScreen.x;
	ContextClick.y = ptScreen.y;
}

// Paint...
void UDebuggerWindow::OnPaint()
{
	guard(UDebuggerWindow::OnPaint);
	PAINTSTRUCT PS;
	HDC hDC = BeginPaint( *this, &PS );
	FillRect( hDC, GetClientRect(), (HBRUSH)(COLOR_BTNFACE+1) );
	EndPaint( *this, &PS );
	unguard;
}


// Evil toolbar tooltip stuff, ignore

#define ID_DEBUG_TOOLBAR 79393
#define TOOLBAR_BUTTON_NUM 10

TBBUTTON tbDEBUGButtons[] = {
	  { 0, ID_DEBUG_GO, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 1, ID_DEBUG_STOPDEBUGGING, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 2, ID_DEBUG_BREAKEXECUTION, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 3, ID_DEBUG_STEPINTO, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 4, ID_DEBUG_STEPOVER, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 5, ID_DEBUG_STEPOVERFUNCTIONCALL, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 6, ID_DEBUG_STEPOUT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 7, ID_DEBUG_RUNTOCURSOR, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
};

struct {
	TCHAR ToolTip[64];
	INT ID;
} ToolTips_CF[] = {
	TEXT("Go"), ID_DEBUG_GO,
	TEXT("Stop Debugging"), ID_DEBUG_STOPDEBUGGING,
	TEXT("Break Execution"), ID_DEBUG_BREAKEXECUTION,
	TEXT("Step Into"), ID_DEBUG_STEPINTO,
	TEXT("Step Over (Entire Statement)"), ID_DEBUG_STEPOVER,
	TEXT("Step Over (Function Calls)"), ID_DEBUG_STEPOVERFUNCTIONCALL,
	TEXT("Step Out"), ID_DEBUG_STEPOUT,
	TEXT("Run to Cursor"), ID_DEBUG_RUNTOCURSOR,
	NULL, 0
};


void UDebuggerWindow::PositionChildControls()
{
	guard(UDebuggerWindow::PositionChildControls);

	FRect CR = GetClientRect();

	RECT R;
	::GetWindowRect( GetDlgItem( hWnd, ID_DEBUG_TOOLBAR ), &R );
	::MoveWindow( GetDlgItem( hWnd, ID_DEBUG_TOOLBAR ), 0, 0, CR.Max.X, R.bottom, TRUE );

	const int TreeViewWidth = 164;
	const int ToolBarHeight = 27;
	const int StatusBarHeight = 22;
	const int WatchWindowHeight = 164;
	const int ConfigAreaWidth = 140;
	const int StackComboHeight = 20;

	// Tree view
	const int TreeViewPosSX = 0;
	const int TreeViewPosSY = ToolBarHeight;
	const int TreeViewPosEX = TreeViewWidth;
	const int TreeViewPosEY = CR.Max.Y - StatusBarHeight;

	// Watch window
	const int WatchWindowPosSX = TreeViewWidth;
	const int WatchWindowPosSY = CR.Max.Y - WatchWindowHeight - StatusBarHeight;
	const int WatchWindowPosEX = CR.Max.X - ConfigAreaWidth;
	const int WatchWindowPosEY = CR.Max.Y - StatusBarHeight;

	// Stack Combo
	const int StackComboPosSX = TreeViewWidth;
	const int StackComboPosSY = WatchWindowPosSY - StackComboHeight;
	const int StackComboPosEX = CR.Max.X - ConfigAreaWidth;
	const int StackComboPosEY = WatchWindowPosSY;

	// Edit
	const int EditPosSX = TreeViewWidth;
	const int EditPosSY = ToolBarHeight;
	const int EditPosEX = CR.Max.X;
	const int EditPosEY = StackComboPosSY;

	// Config
	const int ConfigPosSX = WatchWindowPosEX;
	const int ConfigPosSY = StackComboPosSY;
	const int ConfigPosEX = CR.Max.X;
	const int ConfigPosEY = CR.Max.Y - StatusBarHeight;

	TreeView.MoveWindow(	FRect(TreeViewPosSX,	TreeViewPosSY,	  TreeViewPosEX,	TreeViewPosEY	), TRUE );
	Edit.MoveWindow(		FRect(EditPosSX,		EditPosSY,		  EditPosEX,		EditPosEY		), TRUE );
	Watch.MoveWindow(		FRect(WatchWindowPosSX,	WatchWindowPosSY, WatchWindowPosEX, WatchWindowPosEY), TRUE );
	StackCombo.MoveWindow(	FRect(StackComboPosSX,	StackComboPosSY,  StackComboPosEX,  StackComboPosEY ), TRUE );
	::MoveWindow( hWndStatusBar, 0, CR.Max.Y - StatusBarHeight, CR.Max.X, StatusBarHeight, 0 );
	::MoveWindow( hWndStaticBox, ConfigPosSX, ConfigPosSY, ConfigPosEX - ConfigPosSX, ConfigPosEY - ConfigPosSY, 0 );


	INT StartX = ConfigPosSX + 5;
	INT StartY = ConfigPosSY + 5;
	INT DY = 30;
	HideLocalVars.MoveWindow( FRect(StartX, StartY, StartX + 120, StartY + 24 ), TRUE );
	StartY += DY;
	HideInstanceVars.MoveWindow( FRect(StartX, StartY, StartX + 120, StartY + 24 ), TRUE );
	StartY += DY;
	HideActorVars.MoveWindow( FRect(StartX, StartY, StartX + 130, StartY + 24 ), TRUE );
	StartY += DY;
	HideTypes.MoveWindow( FRect(StartX, StartY, StartX + 120, StartY + 20 ), TRUE );

/*
	Watch.MoveWindow( FRect(164,CR.Max.Y-20-128-24-5,CR.Max.X,CR.Max.Y-20-24-5), TRUE );
	Edit.MoveWindow( FRect(164,27,CR.Max.X,CR.Max.Y-20-128-24-20-5), TRUE );
	StackCombo.MoveWindow( FRect(164,CR.Max.Y-20-128-24-20-5,CR.Max.X,CR.Max.Y-20-24-128-5), TRUE );
	TreeView.MoveWindow( FRect(0,27,164, CR.Max.Y-20-24), TRUE );

	HideLocalVars.MoveWindow( FRect(0,CR.Max.Y-20-24,100,CR.Max.Y-20-24+24), TRUE );
	HideInstanceVars.MoveWindow( FRect(120,CR.Max.Y-20-24,220,CR.Max.Y-20-24+24), TRUE );
	HideActorVars.MoveWindow( FRect(240,CR.Max.Y-20-24,380,CR.Max.Y-20-24+24), TRUE );
	HideTypes.MoveWindow( FRect(380,CR.Max.Y-20-24,540,CR.Max.Y-20-24+24), TRUE );
*/

	if( !::IsWindow( GetDlgItem( hWnd, ID_DEBUG_TOOLBAR )))
		return;

	::InvalidateRect( hWnd, NULL, TRUE );
	//SetStatus( *FString::Printf(TEXT("%i,%i"), CR.Max.X, CR.Max.Y) );
	unguard;
}


void UDebuggerWindow::StackChanged()
{
	int Num = 0;
	if ( IsDebugging == 1 )
	{
		StackCombo.Empty();
		VisibleNodes.Empty();
		VisibleNodes.Add( CallStack->Num() );
		for( INT x = CallStack->Num()-1 ; x >= 0 ; x-- )
		{
			const TCHAR* Test = CallStack->GetNode(x)->StackNode->Node->GetFullName();
			VisibleNodes(Num++) = CallStack->GetNode(x);
			StackCombo.AddString( CallStack->GetNode(x)->StackNode->Node->GetFullName() );
			GLog->Logf(TEXT("%s"),Test);
		}
		VisibleNodes.Shrink();
		StackCombo.SetCurrent(0);
	}
}


void UDebuggerWindow::OnCreate()
{
	guard(UDebuggerWindow::OnCreate);
	WWindow::OnCreate();
	INT X, Y, W, H;

	X = 20;
	Y = 20;
	W = 782;
	H = 560;

	SetMenu( hWnd, LoadMenuIdX(hInstance, IDR_DebugMenu) );

	::MoveWindow( hWnd, X, Y, W, H, TRUE );

	// Set up the main edit control.
	//
	Edit.OpenWindow(1,0);
	UINT Tabs[16];
	for( INT i=0; i<16; i++ )
		Tabs[i]=4*(i+1);
		//Tabs[i]=5*4*(i+1);
	HICON G = LoadIcon( hInstance, TEXT("IDICON_MAINFRAME2"));

	SendMessageX( hWnd, WM_SETICON, (WPARAM)G, ICON_SMALL );
	SendMessageX( Edit.hWnd, EM_SETTABSTOPS, 16, (LPARAM)Tabs );
	Edit.SetFont( (HFONT)GetStockObject(ANSI_FIXED_FONT) );
	SendMessageX( Edit.hWnd, EM_EXLIMITTEXT, 0, 262144 );
	Edit.SetText(TEXT(""));
	SendMessageX( Edit.hWnd, EM_SETTEXTMODE, 0, TM_RICHTEXT | TM_MULTILEVELUNDO );

	Edit.SetReadOnly( TRUE );
	Edit.Parent = this;
	Edit.Snoop = this;

	StackCombo.OpenWindow( 1, 0, CBS_DROPDOWN );
	StackCombo.SelectionChangeDelegate = FDelegate(this,(TDelegate)OnContextChange);
	StackCombo.Snoop = this;
	Watch.OpenWindow( 1, LVS_REPORT | LVS_EDITLABELS  );

	Watch.DblClkDelegate = FDelegate( this, (TDelegate)OnWatchRightClick );
	Watch.Snoop = this;
	hWndToolBar = CreateToolbarEx(
			hWnd, WS_CHILD | WS_BORDER | WS_VISIBLE | CCS_ADJUSTABLE,
			ID_DEBUG_TOOLBAR,
			8,
			hInstance,
			IDB_DEBUG_TOOLBAR,
			(LPCTBBUTTON)&tbDEBUGButtons,
			TOOLBAR_BUTTON_NUM,
			16,16,
			16,16,
			sizeof(TBBUTTON));


		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();

		for( INT tooltip = 0 ; ToolTips_CF[tooltip].ID > 0 ; tooltip++ )
		{
			// Figure out the rectangle for the toolbar button.
			INT index = SendMessageX( hWndToolBar, TB_COMMANDTOINDEX, ToolTips_CF[tooltip].ID, 0 );
			RECT rect;
			SendMessageX( hWndToolBar, TB_GETITEMRECT, index, (LPARAM)&rect);

			ToolTipCtrl->AddTool( hWndToolBar, ToolTips_CF[tooltip].ToolTip, tooltip, &rect );
		}
	hWndStatusBar = CreateStatusWindowW( WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | CCS_BOTTOM, TEXT("Idle"), hWnd, 2 );
	//Status_Simple( hWndStatusBar, 1 );

	hWndStaticBox  = CreateWindowEx(0, TEXT("STATIC"), TEXT("STATICGROUP"), WS_CHILD | WS_VISIBLE|SS_ETCHEDFRAME|SS_SUNKEN, 0, 0, 0, 0, hWnd, NULL, hInstance, 0 );

	HideLocalVars.OpenWindow( 1, 0, 0, 10, 10, TEXT("Hide Local Vars"), 1, 0, 0 );
	HideLocalVars.ClickDelegate = FDelegate(this,(TDelegate)OnHideButtonsClick);
	HideLocalVars.Snoop = this;

	HideInstanceVars.OpenWindow( 1, 0, 0, 10, 10, TEXT("Hide Instance Vars"), 1, 0 );
	HideInstanceVars.ClickDelegate = FDelegate(this,(TDelegate)OnHideButtonsClick);
	HideInstanceVars.Snoop = this;

	HideActorVars.OpenWindow( 1, 0, 0, 10, 10, TEXT("Hide Vars Belonging To:"), 1, 0 );
	HideActorVars.ClickDelegate = FDelegate(this,(TDelegate)OnHideButtonsClick);
	HideActorVars.Snoop = this;

	HideTypes.OpenWindow( 1, 0, 0 );
	HideTypes.SetText( TEXT("Object,Actor") );
	EnableWindow( HideTypes.hWnd, FALSE );
	HideTypes.ChangeDelegate = FDelegate(this,(TDelegate)OnHideTypesChange);
	HideTypes.Snoop = this;

	TreeView.OpenWindow( 1, 1, 0, 0, 1 );
	TreeView.SelChangedDelegate = FDelegate(this, (TDelegate)OnTreeViewSelChanged);
	TreeView.ItemExpandingDelegate = FDelegate(this, (TDelegate)OnTreeViewItemExpanding);
	TreeView.DblClkDelegate = FDelegate(this, (TDelegate)OnTreeViewDblClk);
	TreeView.Snoop = this;
	ToolBar_EnableButton( hWndToolBar, ID_DEBUG_STEPOVER, FALSE );
	ToolBar_EnableButton( hWndToolBar, ID_DEBUG_BREAKEXECUTION, FALSE );
	ToolBar_EnableButton( hWndToolBar, ID_DEBUG_RUNTOCURSOR, FALSE );
	CheckMenuItem(GetMenu(hWnd), ID_TOOLS_TOGGLESHOWOBJECTACTORCLASSES, MF_CHECKED);

	LoadIni();


	LVCOLUMNA lvcol;
	lvcol.mask = LVCF_TEXT | LVCF_WIDTH;
	lvcol.pszText = "Variable Name";
	lvcol.cx = 175;

	SendMessageX( Watch.hWnd, LVM_INSERTCOLUMNA, 0, (LPARAM)(const LPLVCOLUMNA)&lvcol );

	LVCOLUMNA lvcol2;
	lvcol2.mask = LVCF_TEXT | LVCF_WIDTH;
	lvcol2.pszText = "Variable Value";
	lvcol2.cx = 180;

	SendMessageX( Watch.hWnd, LVM_INSERTCOLUMNA, 1, (LPARAM)(const LPLVCOLUMNA)&lvcol2 );


	LVCOLUMNA lvcol3;
	lvcol3.mask = LVCF_TEXT | LVCF_WIDTH;
	lvcol3.pszText = "Belongs To";
	lvcol3.cx = 110;

	SendMessageX( Watch.hWnd, LVM_INSERTCOLUMNA, 2, (LPARAM)(const LPLVCOLUMNA)&lvcol3 );

	unguard;
}

void UDebuggerWindow::RebuildTree()
{
	TreeView.Empty();
	if ( bShowOnlyActorClasses )
		TreeView.AddItem( TEXT("Actor"), NULL, TRUE );
	else
		TreeView.AddItem( TEXT("Object"), NULL, TRUE );
}

void UDebuggerWindow::OnContextChange()
{
	INT Curr = StackCombo.GetCurrent();
	FStackNode* SelStack = VisibleNodes( Curr  );
	WatchNode = SelStack;
	RefreshWatch( WatchNode );
	UpdateInterface( WatchNode );
}


void UDebuggerWindow::OnTreeViewSelChanged( void )
{
	guard(WBrowserActor::OnTreeViewSelChanged);
	const char* copyright = "Copyright 2001, Lucas Alonso. All rights reserved.";
	NMTREEVIEW* pnmtv = (LPNMTREEVIEW)TreeView.LastlParam;
	TCHAR chText[128] = TEXT("\0");
	TVITEM tvi;

	appMemzero( &tvi, sizeof(tvi));
	tvi.hItem = pnmtv->itemNew.hItem;
	tvi.mask = TVIF_TEXT;
	tvi.pszText = chText;
	tvi.cchTextMax = sizeof(chText);

	if( SendMessageX( TreeView.hWnd, TVM_GETITEM, 0, (LPARAM)&tvi) )
	{
		FString Classname = tvi.pszText;
		FString StringQuery = FString::Printf( TEXT("Parent=\"%s\""), *Classname );
		UClass *Class = NULL;
		ParseObject<UClass>(*StringQuery,TEXT("PARENT="),Class,ANY_PACKAGE);
		CurrentSelectedClass = Class;
	}
	copyright = NULL;

	unguard;
}

void UDebuggerWindow::OnTreeViewItemExpanding( void )
{
	NMTREEVIEW* pnmtv = (LPNMTREEVIEW)TreeView.LastlParam;
	TCHAR chText[128] = TEXT("\0");

	TVITEM tvi;

	appMemzero( &tvi, sizeof(tvi));
	tvi.hItem = pnmtv->itemNew.hItem;
	tvi.mask = TVIF_TEXT;
	tvi.pszText = chText;
	tvi.cchTextMax = sizeof(chText);

	// If this item already has children loaded, bail...
	if( SendMessageX( TreeView.hWnd, TVM_GETNEXTITEM, TVGN_CHILD, (LPARAM)pnmtv->itemNew.hItem ) )
		return;

	if( SendMessageX( TreeView.hWnd, TVM_GETITEM, 0, (LPARAM)&tvi) )
		AddChildren( tvi.pszText, pnmtv->itemNew.hItem );
}

// Class.
int CDECL ClassSortCompare( const void *elem1, const void *elem2 )
{
	return appStricmp((*(UClass**)elem1)->GetName(),(*(UClass**)elem2)->GetName());
}

void UDebuggerWindow::AddChildren( const TCHAR* pParentName, HTREEITEM hti )
{
	guard(WBrowserActor::AddChildren);
//	HTREEITEM newhti;
	FString String, StringQuery;
	enum	{MAX_RESULTS=1024};
	int		NumResults = 0;
	UClass	*Results[MAX_RESULTS];

	FString ParentName = pParentName;
	StringQuery = FString::Printf( TEXT("Parent=\"%s\""), *ParentName );
	UClass *Parent = NULL;
	ParseObject<UClass>(*StringQuery,TEXT("PARENT="),Parent,ANY_PACKAGE);

	// Make a list of all child classes.
	for( TObjectIterator<UClass> It; It && NumResults<MAX_RESULTS; ++It )
		if( It->GetSuperClass()==Parent )
			Results[NumResults++] = *It;

	// Sort them by name.
	appQsort( Results, NumResults, sizeof(UClass*), ClassSortCompare );

	// Return the results.
	for( INT i=0; i<NumResults; i++ )
	{
			INT Children = 0;
			for( TObjectIterator<UClass> It; It; ++It )
				if( It->GetSuperClass()==Results[i] )
					Children++;

			TreeView.AddItem( Results[i]->GetName(), hti, Children ? TRUE : FALSE );
	}
	unguard;
}


void UDebuggerWindow::OnTreeViewDblClk( void )
{
	//GCodeFrame->AddClass( GEditor->CurrentClass );
	SetClass( CurrentSelectedClass );
}


void UDebuggerWindow::OpenWindow()
{
	guard(UDebuggerWindow::OpenWindow);
	MdiChild = 0;
	PerformCreateWindowEx
	(
		WS_EX_WINDOWEDGE,
		TEXT("UnrealScript Debugger"),
		WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_MAXIMIZEBOX | WS_MINIMIZEBOX,
		CW_USEDEFAULT,
		CW_USEDEFAULT,
		611,
		538,
		NULL,
		NULL,
		hInstance
	);
	unguard;
}

void UDebuggerWindow::LoadBreakpoints()
{
	OPENFILENAMEA ofn;
	char File[8192];
	memset( File, 0, sizeof(char)*8192 );

	ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
	ofn.lStructSize = sizeof(OPENFILENAMEA);
	ofn.hwndOwner = hWnd;
	ofn.lpstrFile = File;
	ofn.nMaxFile = sizeof(char) * 8192;
	ofn.lpstrDefExt = "udbg";
	ofn.lpstrTitle = "Load Breakpoint Set";
	ofn.lpstrFilter = "Breakpoint Set Files (*.udbg)\0*.udbg\0All Files\0*.*\0\0";
	ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

	// Display the Open dialog box.
	if( GetOpenFileNameA(&ofn) )
	{
		FString FileName = ANSI_TO_TCHAR(File);
		FArchive* BWriter = GFileManager->CreateFileReader( *FileName );
		BreakpointManager->Serialize( *BWriter );
		BWriter->Flush();
		BWriter->Close();
	}
}

void UDebuggerWindow::SaveBreakpoints()
{
	OPENFILENAMEA ofn;
	char File[8192];
	const char* pFilename = "BreakpointSet.udbg";

	strcpy( File, pFilename );

	ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
	ofn.lStructSize = sizeof(OPENFILENAMEA);
	ofn.hwndOwner = hWnd;
	ofn.lpstrFile = File;
	ofn.nMaxFile = sizeof(char) * 8192;
	ofn.lpstrDefExt = "udbg";
	ofn.lpstrTitle = "Save Breakpoint Set";
	ofn.lpstrFilter = "Breakpoint Set Files (*.udbg)\0*.udbg\0All Files\0*.*\0\0";
	ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

	// Display the Open dialog box.
	if( GetSaveFileNameA(&ofn) )
	{
		FString FileName = ANSI_TO_TCHAR(File);
		FArchive* BWriter = GFileManager->CreateFileWriter( *FileName );
		BreakpointManager->Serialize( *BWriter );
		BWriter->Flush();
		BWriter->Close();
	}
}

void FBreakpointManager::Serialize( FArchive& Ar )
{
	// Make sure we're loading the right type of file
	FString Ident(TEXT("UDEBUGV1"));
	Ar << Ident;

	if ( Ident != TEXT("UDEBUGV1") )
	{
		GWarn->Logf(TEXT("Incorrect breakpoint file format!"));
		return;
	}

	if ( Ar.IsLoading() )
	{
		INT Num = 0;
		Ar << Num;
		Breakpoints.Empty();
		GLog->Logf(TEXT("Loading %i breakpoints."), Num );
		Breakpoints.Add( Num );
		for(int i=0;i<Num;i++)
		{
			INT VerifyNum = 0;
			Ar << VerifyNum;

			// File is bad... reset the breakpoints and return
			if ( i != VerifyNum )
			{
				GWarn->Logf(TEXT("Error, Expected %i, got %i."), i, VerifyNum );
				Breakpoints.Empty();
				return;
			}
			Ar << Breakpoints(i).Line;
			Ar << Breakpoints(i).ClassName;
			Ar << Breakpoints(i).IsEnabled;
		}
	}
	else
	{
		INT Num = Breakpoints.Num();
		Ar << Num;
		GLog->Logf(TEXT("Saving %i breakpoints."), Num );
		for(int i=0;i<Num;i++)
		{
			Ar << i;
			Ar << Breakpoints(i).Line;
			Ar << Breakpoints(i).ClassName;
			Ar << Breakpoints(i).IsEnabled;
		}
	}

	FString Term(TEXT("ENDV1"));
	Ar << Term;

	if ( Term != TEXT("ENDV1") )
	{
		GWarn->Logf(TEXT("Unexptected terminator."));
		Breakpoints.Empty();
		return;
	}

}

void UDebuggerWindow::SetStatus( const TCHAR* DebugInfo )
{
	//Status_SetText( hWndStatusBar, 0, 0, DebugInfo );
	SetWindowText( hWndStatusBar, DebugInfo );
	//	StatusBarText = DebugInfo;
	::InvalidateRect( hWnd, NULL, TRUE );
}

INT UDebuggerWindow::OnSysCommand( INT Command )
{
	guard(UDebuggerWindow::OnSysCommand);
	if( Command == SC_CLOSE )
	{
		//GIsRequestingExit = 1;
		bClosing = 1;
		Show(0);
		return 0;
	}
	else
		return WWindow::OnSysCommand( Command );
	unguard;
}

// Rich text format codes for syntax highlighting.
#define RTF_FILE_PREPREPEND	TEXT("{\\rtf1\\ansi\\deff0\\deftab720{\\fonttbl{\\f0\\fswiss MS Sans Serif;}{\\f1\\froman\\fcharset2 Symbol;}{\\f2\\fmodern Courier New;}{\\f3\\fmodern Courier New;}}\r\n") \

#define RTF_FILE_POSTPREPEND   TEXT("\\deflang1033\\pard\\tx0\\tx420\\tx840\\tx1260\\tx1680\\tx2100\\tx2520\\tx2940\\tx3360\\tx3780\\tx4200\\tx4620\\tx5040\\tx5460\\tx5880\\tx6300\\plain\\f2\\fs17\\cf1 ")

#define RTF_FILE_APPEND		TEXT("\\plain\\f3\\fs17\\cf1 \\par }")
#define RTF_LINE_APPEND		TEXT("\r\n\\par ")

#define RTF_TEXT			TEXT("\\plain\\f3\\fs17\\cf0 ")
#define RTF_COMMENT			TEXT("\\plain\\f3\\fs17\\cf1 ")
#define RTF_KEYWORD			TEXT("\\plain\\f3\\fs17\\cf2 ")
#define RTF_LABEL			TEXT("\\plain\\f3\\fs17\\cf3 ")
#define RTF_EXEC			TEXT("\\plain\\f3\\fs17\\cf4 ")
#define RTF_BREAKPOINT		TEXT("\\plain\\f3\\fs17\\cf5\\highlight6 ")
#define RTF_STRINGCONST		TEXT("\\plain\\f3\\fs17\\cf7 ")
#define RTF_NAMECONST		TEXT("\\plain\\f3\\fs17\\cf8 ")
// UnrealScript keywords to highlight.
static inline BYTE CalcHash( const TCHAR* c )
{
	return (appToUpper(c[0]) + appToUpper(c[1]) * 13) & 255;
}
struct FKeyHash
{
	FKeyHash* Next;
	TCHAR* Key;
	FKeyHash( FKeyHash* InNext, TCHAR* InKey )
	:	Next(InNext)
	,	Key(InKey)
	{}
};


void UDebuggerWindow::FormatRichText( UClass* TextClass, const TCHAR* Item, FOutputDevice& Ar )
{
	// Hash the keywords.
	static INT Inited=0;
	static FKeyHash* Hash[256];

	if( !Inited )
	{
		// Init hash table.
		Inited = 1;
		appMemzero( Hash, sizeof(Hash) );

		// Hash the hardcoded names which are tagged for syntax highlighting.
		for( INT i=0; i<FName::GetMaxNames(); i++ )
		{
			FNameEntry *Entry = FName::GetEntry(i);
			if( Entry && Entry->Flags & RF_HighlightedName )
				Hash[CalcHash(Entry->Name)] = new(TEXT("FKeyHash"))FKeyHash( Hash[CalcHash(Entry->Name)], Entry->Name );
		}
	}

	UTextBuffer* Text = new UTextBuffer( Item );
	if( Text && Text->Text.Len() )
	{
		Ar.Log(RTF_FILE_PREPREPEND);
		Ar.Log( *ColorConfig );
		Ar.Log(RTF_FILE_POSTPREPEND);
		// Convert all lines to rtf.
		INT iLine=1;
		const TCHAR* Stream = *Text->Text;
		TCHAR Line[2048];
		INT CommentCount=0;
		while( *Stream != 0 )
		{
			// Get line.
			INT LineComment = 0;
			INT IsLiteral   = 0;
			INT IsQuote     = 0;
			INT QType		= 0;
			INT FirstWord   = 1;
			UBOOL bBreakpoint = BreakpointManager->QueryBreakpoint( TextClass->GetName(), iLine-1 );
			TCHAR *End = Line, *Word = NULL;
			if ( bBreakpoint )
			{
				End = Line + appSprintf( Line, TEXT("%s"), RTF_BREAKPOINT );
			}
			else if( *Stream != '#' )
			{
				End = Line + appSprintf( Line, TEXT("%s"), CommentCount ? RTF_COMMENT : RTF_TEXT );
			}
			else
			{
				IsLiteral = 1;
				End = Line + appSprintf( Line, TEXT("%s"), CommentCount ? RTF_COMMENT : RTF_EXEC );
			}
			for( ; ; )
			{
				// Detect keywords.
				if( CommentCount==0 && !IsQuote && !IsLiteral )
				{
					if( Word==NULL )
					{
						if( appIsAlpha(*Stream) || *Stream=='_' )
						{
							// Got start of word.
							Word = End;
						}
					}
					else
					{
						if( !appIsAlnum(*Stream) && *Stream!='_')
						{
							// Got end of word.
							*End = 0;
							if( FirstWord && *Stream==':' && appStricmp(Word,TEXT("default"))!=0 )
							{
								// Label.
								TCHAR Temp[256];
								appStrcpy( Temp, Word );
								if ( bBreakpoint )
									End = Word + appSprintf(Word,TEXT("%s%s%s"), RTF_BREAKPOINT, Temp, RTF_BREAKPOINT );
								else
									End = Word + appSprintf(Word,TEXT("%s%s%s"), RTF_LABEL, Temp, RTF_TEXT );
							}
							else for( FKeyHash *Item=Hash[CalcHash(Word)]; Item; Item=Item->Next )
							{
								if( appStricmp(Word,Item->Key)==0 )
								{
									// Found a keyword to syntax highlight.
									TCHAR Temp[256];
									appStrcpy( Temp, Word );
									if ( bBreakpoint )
										End = Word + appSprintf(Word,TEXT("%s%s%s"), RTF_BREAKPOINT, Temp, RTF_BREAKPOINT );
									else
										End = Word + appSprintf(Word,TEXT("%s%s%s"), RTF_KEYWORD, Temp, RTF_TEXT );
									break;
								}
							}
							Word      = NULL;
							FirstWord = 0;
						}
					}
				}

				// Detect end of line.
				if( *Stream==0 || *Stream==13 )
					break;

				// Handle quotes and comments.
				if( (*Stream==34 || *Stream==39) && CommentCount==0 && !IsLiteral )
				{
					if( IsQuote && (*Stream==QType||QType==0))
					{
						if ( bBreakpoint )
						{
							if ( QType==34 )
								End += appSprintf(End,TEXT("\"") RTF_BREAKPOINT);
							else
								End += appSprintf(End,TEXT("\'") RTF_BREAKPOINT);
						}
						else
						{
							if ( QType==34 )
								End += appSprintf(End,TEXT("\"") RTF_TEXT);
							else
								End += appSprintf(End,TEXT("\'") RTF_TEXT);
						}

						QType = *Stream;
						IsQuote = !IsQuote;
					}
					else if ( !IsQuote )
					{
						if ( bBreakpoint )
							End += appSprintf(End,RTF_BREAKPOINT TEXT("\""));
						else
						{
							if ( *Stream==34 )
								End += appSprintf(End,RTF_STRINGCONST TEXT("\""));
							else
								End += appSprintf(End,RTF_NAMECONST TEXT("\'"));
						}
						QType = 0;
						IsQuote = !IsQuote;
					}
					Stream++;
				}
				else if( *Stream=='\\' || *Stream=='{' || *Stream=='}' )
				{
					*End++ = '\\';
					*End++ = *Stream++;
				}
				else if( *Stream=='/' && Stream[1]=='/' && CommentCount==0 && !IsQuote )
				{
					if ( bBreakpoint )
						End += appSprintf(End,RTF_BREAKPOINT);
					else
						End += appSprintf(End,RTF_COMMENT);
					*End++ = *Stream++;
					*End++ = *Stream++;
					CommentCount++;
					LineComment=1;
				}
				else if( *Stream=='/' && Stream[1]=='*' )
				{
					if ( bBreakpoint )
						End += appSprintf(End,RTF_BREAKPOINT);
					else
						End += appSprintf(End,RTF_COMMENT);
					*End++ = *Stream++;
					*End++ = *Stream++;
					CommentCount++;
				}
				else if( *Stream=='*' && Stream[1]=='/' )
				{
					*End++ = *Stream++;
					*End++ = *Stream++;
					End += appSprintf(End,RTF_TEXT);
					CommentCount = Max(CommentCount-1,0);
				}
				else
				{
					*End++ = *Stream++;
				}
			}
			if( *Stream==13 ) Stream++;
			if( *Stream==10 ) Stream++;

			// Finish up.
			*End++ = 0;
			CommentCount -= LineComment;

			// Output it.
			Ar.Logf(TEXT("%s%s"), Line, RTF_LINE_APPEND );
			iLine++;
		}
		Ar.Log(RTF_FILE_APPEND);
		delete Text;
	}
}

void WDebugEdit::OnRightButtonDown()
{
	Parent->ShowDebugContextMenu();
}

