/*=============================================================================
	UnDebuggerLogic.cpp: Debugging logic
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Lucas Alonso.
=============================================================================*/

// All this code is in need of a major cleanup, since there's alot of stuff
// leftover from the various partial rewrites it underwent.


#include "DebuggerLaunchPrivate.h"

void FCallStack::UpdateStack( UObject* Debugee, FFrame* FStack, int LineNumber, int InputPos, FString AdditionalInfo )
{	
	if ( AdditionalInfo == TEXT("LATENTPREVSTACK") )
	{
		delete Stack.Pop();
		StackDepth--;
		Parent->StackChanged();		
	}
	else if ( AdditionalInfo == TEXT("PREVSTACK") )
	{
		delete Stack.Pop();
		StackDepth--;
		if ( StackDepth == 0 )
		{
			Parent->ChangeState( new DSIdleState );
			Parent->ClearInterface();
			Parent->IsDebugging = 0;
			Parent->NeedsClear = 0;
		}
		Parent->StackChanged();
	}
	else if ( AdditionalInfo == TEXT("NEWSTACK") || AdditionalInfo == TEXT("LATENTNEWSTACK") )// New stack
	{
		INT VerifyDupe = 1;
		for(int i=0;i<StackDepth;i++)
			if ( Stack(i)->StackNode == FStack )
				VerifyDupe = 0;

		if ( VerifyDupe )
		{
			Stack.AddItem( new FStackNode( Debugee, FStack, LineNumber, InputPos, AdditionalInfo ) );
			StackDepth++;
		}
		
		Parent->StackChanged();
	}
	else if ( TopNode() && TopNode()->StackNode == FStack )
	{
		TopNode()->Update( Debugee, FStack, LineNumber, InputPos, AdditionalInfo );		
	}
}

DSRunToCursor::DSRunToCursor( INT InPos, INT SDepth )
{
	ExpectedPos = InPos;
	EvalDepth = SDepth;
}

// FIXME BROKEN

UBOOL DSRunToCursor::EvaluateCondition()
{
	if ( CurrentPos >= ExpectedPos && EvalDepth == Parent->GetCallStack()->GetStackDepth() )
	{
		return TRUE;
	}
	return FALSE;
}

UBOOL DSStepOut::EvaluateCondition()
{

	if ( Parent->GetCallStack()->GetStackDepth() < EvalDepth )
		return TRUE;

	return FALSE;
}

UBOOL DSStepOverStack::EvaluateCondition()
{
	if ( Parent->GetCallStack()->GetStackDepth() == EvalDepth )
		return TRUE;
	return FALSE;
}


// FIXME BROKEN

DSStepOver::DSStepOver( int InPos, int SDepth )
{
	StartPos = InPos;
	EvalDepth = SDepth;
/*
	char* ch = TCHAR_TO_ANSI( *(Parent->ScriptHash.FindRef( Parent->GetCallStack()->Class->GetFullName() )) );
	char lparen = '(';
	char rparen = ')';
	char semicolon = ';';
	INT iChar = OldPos;
	char* pch = ch + OldPos-1;
	NextStatement = OldPos;
	if ( OldInfo == TEXT("SIMPLEIF") )
	{
		int Balance = 0;
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
		NextStatement = iChar;
	}
	else
	{
		while(1)
		{
			if( *pch == semicolon )
			{
				break;
			}
			iChar++;
			pch++;
		}
		NextStatement = iChar;
	}
	*/
}

// FIXME BROKEN

UBOOL DSStepOver::EvaluateCondition()
{

	//if ( NextStatement == -1 )
	//{
	
	//}
	

	return FALSE;
}

void DSWaitForCondition::Process()
{
	if ( EvaluateCondition() )
	{
		// Condition was MET. We now delegate control to a
		// user-controlled state.
		Parent->ChangeState( new DSWaitForInput() );
		Parent->DebugInfo( CurrentObject, CurrentStack, CurrentInfo, CurrentLine, CurrentPos );
	}
	// Otherwise continue execution.	
}

UBOOL DSWaitForCondition::EvaluateCondition()
{
	return FALSE;
}

void DSWaitForInput::Process()
{
	FDebuggerState::Process();
	bContinue = FALSE;
	PumpMessages();
}


// Return control to the engine
void DSWaitForInput::ContinueExecution()
{
	bContinue = TRUE;
}


// State based input... handles toolbar button presses
void DSWaitForInput::HandleInput( UserAction UserInput )
{
	switch ( UserInput )
	{
	case UA_RunToCursor:
		CHARRANGE sel;

		RichEdit_ExGetSel (Parent->Edit.hWnd, &sel);

		if ( sel.cpMax != sel.cpMin )
		{
			//appMsgf(0,TEXT("Invalid cursor position"));
			
			return;
		}
		Parent->ChangeState( new DSRunToCursor( sel.cpMax, Parent->GetCallStack()->GetStackDepth() ) );
		Parent->IsDebugging = 0;
		ContinueExecution();
		break;
	case UA_Exit:
		Parent->IsDebugging = 0;
		GIsRequestingExit = 1;
		ContinueExecution();
		break;
	case UA_StepInto:
		ContinueExecution();
		Parent->IsDebugging = 1;
		break;
	case UA_StepOver:
		if ( CurrentInfo != TEXT("RETURN") && CurrentInfo != TEXT("RETURNNOTHING") )
		{
			/*
			Parent->ChangeState( new DSStepOver( CurrentObject,
												 CurrentClass,
												 CurrentStack, 
												 CurrentLine, 
												 CurrentPos, 
												 CurrentInfo,
												 Parent->GetCallStack()->GetStackDepth() ) );
			*/

		}
		Parent->IsDebugging = 1;
		ContinueExecution();
		break;
	case UA_StepOverStack:
		if ( CurrentInfo != TEXT("RETURN") && CurrentInfo != TEXT("RETURNNOTHING") )
		{
			Parent->ChangeState( new DSStepOverStack( Parent->GetCallStack()->GetStackDepth()  ) );
		}
		ContinueExecution();
		Parent->IsDebugging = 1;
		break;
	case UA_StepOut:
		Parent->ChangeState( new DSStepOut( Parent->GetCallStack()->GetStackDepth()  ) );
		ContinueExecution();
		Parent->IsDebugging = 1;
		break;
	case UA_Go:
		Parent->ChangeState( new DSIdleState() );
		ContinueExecution();
		Parent->IsDebugging = 0;
	}
}


// Hit breakpoint, wait for user input.

void DSWaitForInput::PumpMessages()
{
	GIsRunning = false;
	HACCEL hAccel = LoadAccelerators (hInstance, MAKEINTRESOURCE(IDR_ACCELERATOR1));
	while( bContinue == FALSE && Parent->bClosing == FALSE )
	{
		guard(MessagePump);
		MSG Msg;
		
		while( PeekMessageX(&Msg,NULL,0,0,PM_REMOVE) )
		{
			if( Msg.message == WM_QUIT )
			{
				GIsRequestingExit = 1;
				ContinueExecution();
			}

			if (!TranslateAccelerator (Parent->hWnd, hAccel, &Msg))
			{
				TranslateMessage (&Msg) ;
				DispatchMessage (&Msg) ;
			}
			else
			{
				guard(TranslateMessage);
				TranslateMessage( &Msg );
				unguardf(( TEXT("%08X %i"), (INT)Msg.hwnd, Msg.message ));

				guard(DispatchMessage);
				DispatchMessageX( &Msg );
				unguardf(( TEXT("%08X %i"), (INT)Msg.hwnd, Msg.message ));
			}
		}
		unguard;
	}
	GIsRunning = true;
}



UClass* GetDebugClass( FFrame* Stack )
{
	UClass* RClass;
	
	// Function?
	RClass = Cast<UClass>( Stack->Node->GetOuter() ); 
	
	// Nope, a state, we need to go one level higher to get the class
	if ( RClass == NULL )
		RClass = Cast<UClass>( Stack->Node->GetOuter()->GetOuter() );

	if ( RClass == NULL )
		RClass = Cast<UClass>( Stack->Node );
	
	// Make sure it's a real class

	check(RClass!=NULL);

	return RClass;
}

// Update state based on a stack node
void FDebuggerState::UpdateStackInfo( FStackNode* CNode )
{
	CurrentObject = CNode->Object;
	CurrentClass  = GetDebugClass( CNode->StackNode );
	CurrentDepth  = Parent->GetCallStack()->GetStackDepth();
	CurrentStack  =	CNode->StackNode;
	CurrentLine   =	CNode->Line;
	CurrentPos	  = CNode->Pos;
	CurrentInfo   =	CNode->Info;
	CurrentStackNode = CNode;
}

// Called when debugging has ended
void UDebuggerWindow::ClearInterface()
{
	if ( NeedsClear == 1 )
	{
		SetDebuggerLine( NULL );
		RefreshWatch( NULL );
		SetText( TEXT("UnrealScript Debugger - Not Debugging") );
		SetStatus( TEXT("Idle") );
		NeedsClear = 0;
	}
}

// Update debugger interface according to the passed stack node. 
// (Set the current class, line, and refresh the watch)
void UDebuggerWindow::UpdateInterface( FStackNode* CNode )
{
	SetClass( GetDebugClass( CNode->StackNode ) );
	SetDebuggerLine( CNode );
	WatchNode = CNode;
	RefreshWatch( CNode );
	
	Show(1);
	
	FString ObjName = CNode->Object->GetFullName();
	FString ClassName = GetDebugClass( CNode->StackNode )->GetName();
	
	SetText( *FString::Printf(TEXT("Debugging - %s in %s"), *ObjName, *ClassName) );
	
	FString MeaningfulInfo;
	FString CurrentInfo = CNode->Info;

	if ( CurrentInfo == TEXT("LET") )
	{
		MeaningfulInfo = TEXT("Variable assignment");
	}
	else if ( CurrentInfo == TEXT("RETURNNOTHING") )
	{
		MeaningfulInfo = TEXT("Falling out of function");
	}
	else if ( CurrentInfo == TEXT("RETURN") )
	{
		MeaningfulInfo = TEXT("Returning a value");
	}
	else if ( CurrentInfo == TEXT("SIMPLEIF") )
	{
		MeaningfulInfo =  TEXT("Evaluating boolean expression");
	}
	else if ( CurrentInfo == TEXT("EFP") )
	{
		MeaningfulInfo =  TEXT("Function call");
	}
	else if ( CurrentInfo == TEXT("ITERATOREFP") )
	{
		MeaningfulInfo = TEXT("Iterator");
	}
	else if ( CurrentInfo == TEXT("FORINIT") )
	{
		MeaningfulInfo = TEXT("For loop init");
	}
	else if ( CurrentInfo == TEXT("FOREVAL") )
	{
		MeaningfulInfo = TEXT("For loop eval");
	}
	else if ( CurrentInfo == TEXT("FORINC") )
	{
		MeaningfulInfo = TEXT("For loop increment");
	}
	else if ( CurrentInfo == TEXT("ASSERT") )
	{
		MeaningfulInfo = TEXT("Asserting");
	}
	else if ( CurrentInfo == TEXT("SWITCH") )
	{
		MeaningfulInfo = TEXT("Switching");
	}	
	else
	{
		MeaningfulInfo = TEXT("Unknown Opcode");
	}
	SetStatus( *FString::Printf(TEXT("%s on line %i in %s"), *MeaningfulInfo, CNode->Line, CNode->StackNode->Node->GetName() ) );	
}

// Default Process() for states... update the debugger ui every debug info
void FDebuggerState::Process()
{
	Parent->UpdateInterface( CurrentStackNode );
}

// Process any pending debugger states
void UDebuggerWindow::ProcessPendingState()
{
	if ( PendingState != NULL )
	{
		if ( CurrentState != NULL )
		{
			delete CurrentState;
			CurrentState = NULL;
		}
		CurrentState = PendingState;
		PendingState = NULL;
		CurrentState->SetParent( this );
	}
}

// Set a pending state change
void UDebuggerWindow::ChangeState( FDebuggerState* NewState )
{
	PendingState = NewState;
}

// !! Main entry point into the debugger.

void UDebuggerWindow::DebugInfo( UObject* Debugee, FFrame* Stack, FString InfoType, int LineNumber, int InputPos )
{
	// Wierd devastation fix
	if ( Stack->Node->IsA( UClass::StaticClass() ) )
		return;
	
	// Process any waiting states
	ProcessPendingState();
	
	if ( CallStack && BreakpointManager && CurrentState )
	{
		if ( !GIsRequestingExit && !bClosing )
		{
			CallStack->UpdateStack( Debugee, Stack, LineNumber, InputPos, InfoType );
			// If there's a breakpoint on this line, stop all other stuff going on, and see what the user wants to do		
			if ( InfoType != TEXT("OPEREFP") 
				&& InfoType != TEXT("PREVSTACK") 
				&& InfoType != TEXT("NEWSTACK")
				&& InfoType != TEXT("LATENTPREVSTACK") 
				&& InfoType != TEXT("LATENTNEWSTACK") )
			{
				if ( CallStack->GetStackDepth() > 0 )
				{
					if ( BreakpointManager->QueryBreakpoint( GetDebugClass( Stack )->GetName(), LineNumber-1 ) )
					{
						IsDebugging = 1;
						NeedsClear = 1;
						StackChanged();
						ChangeState( new DSWaitForInput );
						ProcessPendingState();
					}
					
					// Let the state work its magic, chances are we won't return here for a while...
					CurrentState->UpdateStackInfo( CallStack->TopNode() );
					CurrentState->Process();
				}
			}
		}
	}
}


UBOOL FBreakpointManager::QueryBreakpoint( FString sClassName, INT sLine )
{
	for(int i=0;i<Breakpoints.Num();i++)
	{
			if ( Breakpoints(i).IsEnabled && Breakpoints(i).ClassName == sClassName && Breakpoints(i).Line == sLine )
				return TRUE;
	}
	
	return FALSE;
}

void FBreakpointManager::SetBreakpoint( FString sClassName, INT sLine )
{
	for(int i=0;i<Breakpoints.Num();i++)
	{
			if ( Breakpoints(i).ClassName == sClassName && Breakpoints(i).Line == sLine )
				return;
	}

	Breakpoints( Breakpoints.AddZeroed() ) = FBreakpoint( sClassName, sLine );
}

void FBreakpointManager::RemoveBreakpoint( FString sClassName, INT sLine )
{
	for(int i=0;i<Breakpoints.Num();i++)
	{
			if ( Breakpoints(i).ClassName == sClassName && Breakpoints(i).Line == sLine )
				Breakpoints.Remove(i);
	}
}

void FBreakpointManager::ToggleBreakpoint( FString sClassName, INT sLine )
{

}