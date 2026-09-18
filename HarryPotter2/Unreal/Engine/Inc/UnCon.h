/*=============================================================================
	UnCon.h: UConsole game-specific definition
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Contains routines for: Messages, menus, status bar
=============================================================================*/

/*------------------------------------------------------------------------------
	UConsole definition.
------------------------------------------------------------------------------*/

//
// Viewport console.
//
struct UConsole_eventMessage_Parms
{
	class APlayerReplicationInfo* PRI;
	FString S;
	class AZoneInfo* PZone;
	FName N;
};
struct UConsole_eventConnectFailure_Parms
{
    FString FailCode;
    FString URL;
};
struct UConsole_eventDrawLevelInfo_Parms
{
    UCanvas* Canvas;
    FString URL;
};
class ENGINE_API UConsole : public UObject, public FOutputDevice
{
	DECLARE_CLASS(UConsole,UObject,CLASS_Transient,Engine)

	// Constructor.
	UConsole();
	void StaticConstructor();

	// UConsole interface.
	virtual void _Init( UViewport* Viewport );
	virtual void PreRender( FSceneNode* Frame );
	virtual void PostRender( FSceneNode* Frame );
	virtual void Serialize( const TCHAR* Data, EName MsgType );
	virtual UBOOL GetDrawWorld();

	// Natives.
	DECLARE_FUNCTION(execConsoleCommand);
	DECLARE_FUNCTION(execSaveTimeDemo);
	DECLARE_FUNCTION(execCreateNativeFont);

	// Script events.
    void eventMessage(class APlayerReplicationInfo* PRI, const FString& S, class AZoneInfo* PZone, FName Name)
    {
		UConsole_eventMessage_Parms Parms;
		Parms.PRI=PRI;
        Parms.S=S;
		Parms.PZone=PZone;
		Parms.N=Name;
        ProcessEvent(FindFunctionChecked(NAME_Message),&Parms);
    }
    void eventTick(FLOAT DeltaTime)
    {
        struct {FLOAT DeltaTime; } Parms;
        Parms.DeltaTime=DeltaTime;
        ProcessEvent(FindFunctionChecked(ENGINE_Tick),&Parms);
    }
    void eventVideoChange()
    {
        ProcessEvent(FindFunctionChecked(NAME_VideoChange),NULL);
    }
    void eventPostRender(class UCanvas* C)
    {
        struct {class UCanvas* C; } Parms;
        Parms.C=C;
        ProcessEvent(FindFunctionChecked(ENGINE_PostRender),&Parms);
    }
    void eventPreRender(class UCanvas* C)
    {
        struct {class UCanvas* C; } Parms;
        Parms.C=C;
        ProcessEvent(FindFunctionChecked(ENGINE_PreRender),&Parms);
    }
    DWORD eventKeyType(BYTE Key)
    {
        struct {BYTE Key; DWORD ReturnValue; } Parms;
        Parms.Key=Key;
        Parms.ReturnValue=0;
        ProcessEvent(FindFunctionChecked(NAME_KeyType),&Parms);
        return Parms.ReturnValue;
    }
    DWORD eventKeyEvent(BYTE Key, BYTE Action, FLOAT Delta)
    {
        struct {BYTE Key; BYTE Action; FLOAT Delta; DWORD ReturnValue; } Parms;
        Parms.Key=Key;
        Parms.Action=Action;
        Parms.Delta=Delta;
        Parms.ReturnValue=0;
        ProcessEvent(FindFunctionChecked(NAME_KeyEvent),&Parms);
        return Parms.ReturnValue;
    }
    void eventNotifyLevelChange()
    {
        ProcessEvent(FindFunctionChecked(NAME_NotifyLevelChange),NULL);
    }
    void eventConnectFailure(const FString& FailCode, const FString& URL)
    {
        UConsole_eventConnectFailure_Parms Parms;
        Parms.FailCode=FailCode;
        Parms.URL=URL;
        ProcessEvent(FindFunctionChecked(NAME_ConnectFailure),&Parms);
    }
    void eventDrawLevelInfo(UCanvas* Canvas, const FString& URL)
    {
        UConsole_eventDrawLevelInfo_Parms Parms;
        Parms.Canvas=Canvas;
        Parms.URL=URL;
        ProcessEvent(FindFunctionChecked(TEXT("DrawLevelInfo")),&Parms);
    }
	UBOOL IsTimeDemo()
	{
		return bTimeDemo;
	}
	UBOOL DrewWorld()
	{
		return bDrewWorld;
	}
private:
	// Constants.
	enum {MAX_BORDER     = 6};
	enum {MAX_LINES		 = 64};
	enum {MAX_HISTORY	 = 16};

	// Variables.
    class UViewport* Viewport GCC_PACK(4);
    INT HistoryTop;
    INT HistoryBot;
    INT HistoryCur;
    FStringNoInit TypedStr GCC_PACK(4);
    FStringNoInit History[16] GCC_PACK(4);
    INT Scrollback;
    INT numLines;
    INT TopLine;
    INT TextLines;
    FLOAT MsgTime;
    FLOAT MsgTickTime;
    FStringNoInit MsgText[64] GCC_PACK(4);
    FName MsgType[64] GCC_PACK(4);
    class APlayerReplicationInfo* MsgPlayer[64] GCC_PACK(4);
    FLOAT MsgTick[64];
    INT BorderSize;
    INT ConsoleLines;
    INT BorderLines;
    INT BorderPixels;
    FLOAT ConsolePos;
    FLOAT ConsoleDest;
    FLOAT FrameX;
    FLOAT FrameY;
    class UTexture* ConBackground GCC_PACK(4);
    class UTexture* Border GCC_PACK(4);
    BITFIELD bNoStuff:1 GCC_PACK(4);
    BITFIELD bTyping:1;
    BITFIELD bNoDrawWorld:1;
    BITFIELD bDrewWorld:1;
    BITFIELD bTimeDemo:1;
    BITFIELD bStartTimeDemo:1;
    BITFIELD bRestartTimeDemo:1;
	BITFIELD bSaveTimeDemoToFile:1;
    FLOAT StartTime GCC_PACK(4);
    FLOAT ExtraTime;
    FLOAT LastFrameTime;
    FLOAT LastSecondStartTime;
    INT FrameCount;
    INT LastSecondFrameCount;
    FLOAT MinFPS;
    FLOAT MaxFPS;
    FLOAT LastSecFPS;
	class UFont* Font GCC_PACK(4);
public:
	float FadeoutTime, FadeinTime;
    FStringNoInit LoadingMessage GCC_PACK(4);
    FStringNoInit SavingMessage GCC_PACK(4);
    FStringNoInit ConnectingMessage GCC_PACK(4);
    FStringNoInit PausedMessage GCC_PACK(4);
    FStringNoInit PrecachingMessage GCC_PACK(4);
    FStringNoInit FrameRateText GCC_PACK(4);
    FStringNoInit AvgText GCC_PACK(4);
    FStringNoInit LastSecText GCC_PACK(4);
    FStringNoInit MinText GCC_PACK(4);
    FStringNoInit MaxText GCC_PACK(4);
    FStringNoInit fpsText GCC_PACK(4);
    FStringNoInit SecondsText GCC_PACK(4);
    FStringNoInit FramesText GCC_PACK(4);
};

/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/

