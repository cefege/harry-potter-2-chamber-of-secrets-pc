/*=============================================================================
	UnCon.cpp: Implementation of UConsole class
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"

/*------------------------------------------------------------------------------
	UConsole object implementation.
------------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UConsole);

/*------------------------------------------------------------------------------
	Console.
------------------------------------------------------------------------------*/

//
// Constructor.
//
UConsole::UConsole()
{}
void UConsole::StaticConstructor()
{
	guard(UConsole::StaticConstructor);
	new(GetClass(),TEXT("Viewport"),RF_Public)UObjectProperty(CPP_PROPERTY(Viewport),TEXT("Console"),CPF_Transient,UViewport::StaticClass());
	new(GetClass(),TEXT("TypedStr"),RF_Public)UStrProperty(CPP_PROPERTY(TypedStr),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("HistoryTop"),RF_Public)UIntProperty(CPP_PROPERTY(HistoryTop),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("HistoryBot"),RF_Public)UIntProperty(CPP_PROPERTY(HistoryBot),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("HistoryCur"),RF_Public)UIntProperty(CPP_PROPERTY(HistoryCur),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("History"),RF_Public)UStrProperty(CPP_PROPERTY(History),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MsgText"),RF_Public)UStrProperty(CPP_PROPERTY(MsgText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("Scrollback"),RF_Public)UIntProperty(CPP_PROPERTY(Scrollback),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("numLines"),RF_Public)UIntProperty(CPP_PROPERTY(numLines),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("TopLine"),RF_Public)UIntProperty(CPP_PROPERTY(TopLine),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("TextLines"),RF_Public)UIntProperty(CPP_PROPERTY(TextLines),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MsgTime"),RF_Public)UFloatProperty(CPP_PROPERTY(MsgTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MsgTickTime"),RF_Public)UFloatProperty(CPP_PROPERTY(MsgTickTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MsgType"),RF_Public)UNameProperty(CPP_PROPERTY(MsgType),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MsgPlayer"),RF_Public)UObjectProperty(CPP_PROPERTY(MsgPlayer),TEXT("Console"),CPF_Transient,APlayerReplicationInfo::StaticClass());
	new(GetClass(),TEXT("ConBackground"),RF_Public)UObjectProperty(CPP_PROPERTY(ConBackground),TEXT("Console"),CPF_Transient,UTexture::StaticClass());
	new(GetClass(),TEXT("MsgTick"),RF_Public)UFloatProperty(CPP_PROPERTY(MsgTick),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("BorderSize"),RF_Public)UIntProperty(CPP_PROPERTY(BorderSize),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("ConsoleLines"),RF_Public)UIntProperty(CPP_PROPERTY(ConsoleLines),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("BorderLines"),RF_Public)UIntProperty(CPP_PROPERTY(BorderLines),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("BorderPixels"),RF_Public)UIntProperty(CPP_PROPERTY(BorderPixels),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("ConsolePos"),RF_Public)UFloatProperty(CPP_PROPERTY(ConsolePos),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("ConsoleDest"),RF_Public)UFloatProperty(CPP_PROPERTY(ConsoleDest),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FrameX"),RF_Public)UFloatProperty(CPP_PROPERTY(FrameX),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FrameY"),RF_Public)UFloatProperty(CPP_PROPERTY(FrameY),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("Border"),RF_Public)UObjectProperty(CPP_PROPERTY(Border),TEXT("Console"),CPF_Transient,UTexture::StaticClass());
	new(GetClass(),TEXT("Font"),RF_Public)UObjectProperty(CPP_PROPERTY(Font),TEXT("Console"),CPF_Transient,UFont::StaticClass());
	new(GetClass(),TEXT("LoadingMessage"),RF_Public)UStrProperty(CPP_PROPERTY(LoadingMessage),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("SavingMessage"),RF_Public)UStrProperty(CPP_PROPERTY(SavingMessage),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("StartTime"),RF_Public)UFloatProperty(CPP_PROPERTY(StartTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("ExtraTime"),RF_Public)UFloatProperty(CPP_PROPERTY(ExtraTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("LastFrameTime"),RF_Public)UFloatProperty(CPP_PROPERTY(LastFrameTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("LastSecondStartTime"),RF_Public)UFloatProperty(CPP_PROPERTY(LastSecondStartTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FrameCount"),RF_Public)UIntProperty(CPP_PROPERTY(FrameCount),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("LastSecondFrameCount"),RF_Public)UIntProperty(CPP_PROPERTY(LastSecondFrameCount),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MinFPS"),RF_Public)UFloatProperty(CPP_PROPERTY(MinFPS),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MaxFPS"),RF_Public)UFloatProperty(CPP_PROPERTY(MaxFPS),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("LastSecFPS"),RF_Public)UFloatProperty(CPP_PROPERTY(LastSecFPS),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("ConnectingMessage"),RF_Public)UStrProperty(CPP_PROPERTY(ConnectingMessage),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("PausedMessage"),RF_Public)UStrProperty(CPP_PROPERTY(PausedMessage),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FadeoutTime"),RF_Public)UFloatProperty(CPP_PROPERTY(FadeoutTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FadeinTime"),RF_Public)UFloatProperty(CPP_PROPERTY(FadeinTime),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("PrecachingMessage"),RF_Public)UStrProperty(CPP_PROPERTY(PrecachingMessage),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FrameRateText"),RF_Public)UStrProperty(CPP_PROPERTY(FrameRateText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("AvgText"),RF_Public)UStrProperty(CPP_PROPERTY(AvgText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("LastSecText"),RF_Public)UStrProperty(CPP_PROPERTY(LastSecText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MinText"),RF_Public)UStrProperty(CPP_PROPERTY(MinText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("MaxText"),RF_Public)UStrProperty(CPP_PROPERTY(MaxText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("fpsText"),RF_Public)UStrProperty(CPP_PROPERTY(fpsText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("SecondsText"),RF_Public)UStrProperty(CPP_PROPERTY(SecondsText),TEXT("Console"),CPF_Transient);
	new(GetClass(),TEXT("FramesText"),RF_Public)UStrProperty(CPP_PROPERTY(FramesText),TEXT("Console"),CPF_Transient);
	unguard;
}


//
// Init console.
//
void UConsole::_Init( UViewport* InViewport )
{
	guard(UConsole::_Init);
	VERIFY_CLASS_SIZE(UConsole);

	// Set properties.
	Viewport		= InViewport;
	TopLine			= MAX_LINES-1;
	BorderSize		= 1; 

	// Init scripting.
	InitExecution();

	// Start console log.
	Logf(LocalizeGeneral("Engine",TEXT("Core")));
	Logf(LocalizeGeneral("Copyright",TEXT("Core")));
	Logf(TEXT(" "));
	Logf(TEXT(" "));

	unguard;
}

/*------------------------------------------------------------------------------
	Viewport console output.
------------------------------------------------------------------------------*/

//
// Print a message on the playing screen.
// Time = time to keep message going, or 0=until next message arrives, in 60ths sec
//
void UConsole::Serialize( const TCHAR* Data, EName ThisType )
{
	guard(UConsole::Serialize);
	eventMessage( 0, Data, 0, ThisType );
	unguard;
}

void UConsole::execConsoleCommand( FFrame& Stack, RESULT_DECL )
{
	guardSlow(UConsole::execConsoleCommand);

	P_GET_STR(S);
	P_FINISH;

	*(DWORD*)Result = Viewport->Exec( *S, *this );

	unguardexecSlow;
}
IMPLEMENT_FUNCTION( UConsole, INDEX_NONE, execConsoleCommand );

void UConsole::execSaveTimeDemo( FFrame& Stack, RESULT_DECL )
{
	guard(UConsole::execSaveTimeDemo);
	P_GET_STR(S);
	P_FINISH;
	appSaveStringToFile( S, TEXT("fps.txt"), GFileManager );
	unguardexec;
}
IMPLEMENT_FUNCTION( UConsole, INDEX_NONE, execSaveTimeDemo );

void UConsole::execCreateNativeFont( FFrame& Stack, RESULT_DECL )
{
	guard(UConsole::execCreateNativeFont);
	P_GET_STR(FontName);
	P_GET_INT(Height);
	P_FINISH;

	*(UFont**)Result = Viewport->CreateNativeFont( *FontName, Height );
	unguardexec;
}
IMPLEMENT_FUNCTION( UConsole, -1, execCreateNativeFont );

/*------------------------------------------------------------------------------
	Rendering.
------------------------------------------------------------------------------*/

UBOOL UConsole::GetDrawWorld()
{
	guard(UConsole::GetDrawWorld);

	return !bNoDrawWorld;
	unguard;
}

//
// Called before rendering the world view.  Here, the
// Viewport console code can affect the screen's Viewport,
// for example by shrinking the view according to the
// size of the status bar.
//
FSceneNode SavedFrame;
void UConsole::PreRender( FSceneNode* Frame )
{
	guard(UConsole::PreRender);

	// Prevent status redraw due to changing.
	eventTick( Viewport->CurrentTime - Viewport->LastUpdateTime );

	// Save the Viewport.
	SavedFrame = *Frame;

	// Compute new status info.
	BorderLines		= 0;
	BorderPixels	= 0;
	ConsoleLines	= 0;

	// Compute sizing of all visible status bar components.
	if( ConsolePos > 0.f )
	{
		// Show console.
		ConsoleLines = appRound(Min(ConsolePos * (FLOAT)Frame->Y, (FLOAT)Frame->Y));
	}

	if( BorderSize>=2 )
	{
		// Encroach on screen area.
		FLOAT Fraction = (FLOAT)(BorderSize-1) / (FLOAT)(MAX_BORDER-1);

		BorderLines = appRound(Min((FLOAT)Frame->Y * 0.25f * Fraction,(FLOAT)Frame->Y));
		BorderLines = ::Max(0,BorderLines);
		Frame->Y -= 2 * BorderLines;

		BorderPixels = appRound(Min((FLOAT)Frame->X * 0.25f * Fraction,(FLOAT)Frame->X)) & ~3;
		Frame->X -= 2 * BorderPixels;
	}

	Frame->XB += BorderPixels;
	Frame->YB += BorderLines;
	Frame->ComputeRenderSize();

	unguard;
}

//
// Refresh the player console on the specified Viewport.  This is called after
// all in-game graphics are drawn in the rendering loop, and it overdraws stuff
// with the status bar, menus, and chat text.
//
void UConsole::PostRender( FSceneNode* Frame )
{
	guard(UConsole::PostRender);
	
	*Frame = SavedFrame;
	FrameX = Frame->X;
	FrameY = Frame->Y;
	bDrewWorld = !bNoDrawWorld;

	unguard;
}

/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/
