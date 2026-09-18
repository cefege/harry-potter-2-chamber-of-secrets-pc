/*=============================================================================
	UnGame.cpp: Unreal game engine.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnNet.h"


#if defined(__GNUC__)
extern void HP2ActorSlotDumpPostDeserialize( ULevel* Level ) __attribute__((weak));
#else
extern void HP2ActorSlotDumpPostDeserialize( ULevel* Level );
#endif

extern void HP2ActorTransitionPreBeginBefore( AActor* Actor );
extern void HP2ActorTransitionPreBeginAfter( AActor* Actor );
/*-----------------------------------------------------------------------------
	Object class implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UGameEngine);

/*-----------------------------------------------------------------------------
	cleanup!!
-----------------------------------------------------------------------------*/

void UGameEngine::PaintProgress()
{
	guard(PaintProgress);

	if( GIsDrawing )
		return;
	FPlane LoadFog(0.f,0.1f,0.25f,0.2f);
	UViewport* Viewport=Client->Viewports(0);
	Exchange(Viewport->Actor->FlashFog,LoadFog);
	Draw( Viewport );
	Exchange(Viewport->Actor->FlashFog,LoadFog);

	unguard;
}

INT UGameEngine::ChallengeResponse( INT Challenge )
{
	guard(UGameEngine::ChallengeResponse);
	return (Challenge*237) ^ (0x93fe92Ce) ^ (Challenge>>16) ^ (Challenge<<16);
	unguard;
}

void UGameEngine::UpdateConnectingMessage()
{
	guard(UGameEngine::UpdateConnectingMessage);
	if( GPendingLevel && Client && Client->Viewports.Num() )
	{
		APlayerPawn* Actor = Client->Viewports(0)->Actor;
		if( Actor->ProgressTimeOut<Actor->Level->TimeSeconds )
		{
			TCHAR Msg1[256], Msg2[256];
			if( GPendingLevel->DemoRecDriver )
			{
				appSprintf( Msg1, TEXT("") );
				appSprintf( Msg2, *GPendingLevel->URL.Map );
			}
			else
			{
				appSprintf( Msg1, LocalizeProgress("ConnectingText") );
				appSprintf( Msg2, LocalizeProgress("ConnectingURL"), *GPendingLevel->URL.Host, *GPendingLevel->URL.Map );
			}
			SetProgress( Msg1, Msg2, 60.f );
		}
	}
	unguard;
}
void UGameEngine::BuildServerMasterMap( UNetDriver* NetDriver, ULevel* InLevel )
{
	guard(UGameEngine::BuildServerMasterMap);
	check(NetDriver);
	check(InLevel);
	BeginLoad();
	{
		// Init LinkerMap.
		check(InLevel->GetLinker());
		NetDriver->MasterMap->AddLinker( InLevel->GetLinker() );

		// Load server-required packages.
		for( INT i=0; i<ServerPackages.Num(); i++ )
		{
			debugf( TEXT("Server Package: %s"), *ServerPackages(i) );
			ULinkerLoad* Linker = GetPackageLinker( NULL, *ServerPackages(i), LOAD_NoFail, NULL, NULL );
			if( NetDriver->MasterMap->AddLinker( Linker )==INDEX_NONE )
				debugf( TEXT("   (server-side only)") );
		}

		// Add GameInfo's package to map.
		check(InLevel->GetLevelInfo());
		check(InLevel->GetLevelInfo()->Game);
		check(InLevel->GetLevelInfo()->Game->GetClass()->GetLinker());
		NetDriver->MasterMap->AddLinker( InLevel->GetLevelInfo()->Game->GetClass()->GetLinker() );

		// Precompute linker info.
		NetDriver->MasterMap->Compute();
	}
	EndLoad();
	unguard;
}

/*-----------------------------------------------------------------------------
	Game init and exit.
-----------------------------------------------------------------------------*/
static INT GRepairSaveSlot = INDEX_NONE;
static UBOOL GRepairSavePending = 0;

static UBOOL ParseCommandLineSaveSlot( const TCHAR* CmdLine, const TCHAR* Prefix, INT& Slot )
{
	if( !CmdLine || !Prefix )
		return 0;

	const INT PrefixLen = appStrlen(Prefix);
	while( *CmdLine )
	{
		while( *CmdLine==' ' || *CmdLine=='\t' || *CmdLine=='\r' || *CmdLine=='\n' || *CmdLine==',' )
			CmdLine++;
		if( !*CmdLine )
			break;

		UBOOL QuotedToken = *CmdLine=='"';
		const TCHAR* Token = CmdLine + QuotedToken;
		const TCHAR* TokenEnd = Token;
		if( QuotedToken )
		{
			while( *TokenEnd && *TokenEnd!='"' )
				TokenEnd++;
			if( !*TokenEnd )
				return 0;
			CmdLine = TokenEnd+1;
		}
		else
		{
			while( *TokenEnd && *TokenEnd!=' ' && *TokenEnd!='\t' && *TokenEnd!='\r' && *TokenEnd!='\n' && *TokenEnd!=',' )
				TokenEnd++;
			CmdLine = TokenEnd;
		}

		if( TokenEnd-Token < PrefixLen || appStrnicmp(Token,Prefix,PrefixLen)!=0 )
			continue;

		const TCHAR* Value = Token + PrefixLen;
		if( Value<TokenEnd && *Value=='"' )
		{
			if( TokenEnd[-1]!='"' )
				return 0;
			Value++;
			TokenEnd--;
		}
		if( Value==TokenEnd )
			return 0;

		INT ParsedSlot = 0;
		while( Value<TokenEnd )
		{
			if( !appIsDigit(*Value) )
				return 0;
			const INT Digit = *Value++ - '0';
			if( ParsedSlot > (MAXINT-Digit)/10 )
				return 0;
			ParsedSlot = ParsedSlot*10 + Digit;
		}
		Slot = ParsedSlot;
		return 1;
	}
	return 0;
}

UBOOL appConsumeCommandLineLoadSlot( const TCHAR* CmdLine, INT& Slot )
{
	static UBOOL Consumed = 0;
	if( Consumed )
		return 0;

	INT ParsedSlot;
	const UBOOL RepairSave = ParseCommandLineSaveSlot(CmdLine,TEXT("-REPAIRSAVE="),ParsedSlot);
	if( !RepairSave && !ParseCommandLineSaveSlot(CmdLine,TEXT("-LOAD="),ParsedSlot) )
		return 0;

	Consumed = 1;
	GRepairSaveSlot = RepairSave ? ParsedSlot : INDEX_NONE;
	Slot = ParsedSlot;
	return 1;
}

UBOOL appFormatLoadGameURL( INT Slot, TCHAR* Out, INT OutCapacity )
{
	if( Slot < 0 || !Out || OutCapacity < 18 )
		return 0;
	appSprintf(Out,TEXT("?load=%i"),Slot);
	return 1;
}


//
// Construct the game engine.
//
UGameEngine::UGameEngine()
: LastURL(TEXT(""))
, ServerActors( E_NoInit )
, ServerPackages( E_NoInit )
{}

//
// Class creator.
//
void UGameEngine::StaticConstructor()
{
	guard(UGameEngine::StaticConstructor);

	UArrayProperty* A = new(GetClass(),TEXT("ServerActors"),RF_Public)UArrayProperty( CPP_PROPERTY(ServerActors), TEXT("Settings"), CPF_Config );
	A->Inner = new(A,TEXT("StrProperty0"),RF_Public)UStrProperty;

	UArrayProperty* B = new(GetClass(),TEXT("ServerPackages"),RF_Public)UArrayProperty( CPP_PROPERTY(ServerPackages), TEXT("Settings"), CPF_Config );
	B->Inner = new(B,TEXT("StrProperty0"),RF_Public)UStrProperty;

	new(GetClass(),TEXT("FrameRateLimit"),RF_Public)UFloatProperty( CPP_PROPERTY(FrameRateLimit), TEXT("GameEngine"), CPF_Config );

	unguard;
}

//
// Initialize the game engine.
//
void UGameEngine::Init()
{
	guard(UGameEngine::Init);
	check(sizeof(*this)==GetClass()->GetPropertiesSize());

	// Call base.
	UEngine::Init();

	// Init variables.
	GLevel = NULL;
	GViewport = NULL;
	GIsDrawing = false;

	// Delete temporary files in cache.
	appCleanFileCache();

	// If not a dedicated server.
	if( GIsClient )
	{	
		// Init client.
		UClass* ClientClass = StaticLoadClass( UClient::StaticClass(), NULL, TEXT("ini:Engine.Engine.ViewportManager"), NULL, LOAD_NoFail, NULL );
		Client = ConstructObject<UClient>( ClientClass );
		Client->Init( this );

		// Init rendering.
		UClass* RenderClass = StaticLoadClass( URenderBase::StaticClass(), NULL, TEXT("ini:Engine.Engine.Render"), NULL, LOAD_NoFail, NULL );
		Render = ConstructObject<URenderBase>( RenderClass );
		Render->Init( this );
	}

	// Load the entry level.
	FString Error;
	if( Client )
	{
//		FPushMemTag Push(TEXT("LoadEntry"));
		if( !LoadMap( FURL(TEXT("Entry")), NULL, NULL, Error ) )
			appErrorf( LocalizeError("FailedBrowse"), TEXT("Entry"), *Error );
		Exchange( GLevel, GEntry );
	}

	// Create default URL.
	FURL DefaultURL;
	DefaultURL.LoadURLConfig( TEXT("DefaultPlayer"), TEXT("User") );

	// Enter initial world.
	TCHAR Parm[4096]=TEXT("");
	const TCHAR* Tmp = appCmdLine();
	if
	(	!ParseToken( Tmp, Parm, ARRAY_COUNT(Parm), 0 )
	||	(appStricmp(Parm,TEXT("SERVER"))==0 && !ParseToken( Tmp, Parm, ARRAY_COUNT(Parm), 0 ))
	||	Parm[0]=='-' )
		appStrcpy( Parm, *FURL::DefaultLocalMap );

	FString ReplayFile;
	if( Parse( appCmdLine(), TEXT("-REPLAY="), ReplayFile ) )
	{
		// Start replay if specified.
		if( Replay.Replay( *ReplayFile, *GLog ) )
			// Retrieve the URL from it.
			appStrcpy( Parm, *Replay.GetURLStr() );
	}
	else if( Parse( appCmdLine(), TEXT("-RECORD="), ReplayFile ) )
	{
		// Start recording, with this URL.
		Replay.Record( *ReplayFile, Parm, *GLog );
	}
	

	if( !appStrstr(Parm,TEXT("://")) && Parm[0]!='?' )
	{
		TCHAR FileToken[ARRAY_COUNT(Parm)];
		if( appFilePathToFURLToken(Parm,FileToken,ARRAY_COUNT(FileToken)) )
			appStrcpy(Parm,FileToken);
	}

	FURL URL( &DefaultURL, Parm, TRAVEL_Partial );
	if( !URL.Valid )
		appErrorf( LocalizeError("InvalidUrl"), Parm );
		
	UBOOL Success = Browse( URL, NULL, Error );

	// If waiting for a network connection, go into the starting level.
	if( !Success && Error==TEXT("") && appStricmp( Parm, *FURL::DefaultLocalMap )!=0 )
		Success = Browse( FURL(&DefaultURL,*FURL::DefaultLocalMap,TRAVEL_Partial), NULL, Error );

	// Handle failure.
	if( !Success )
		appErrorf( LocalizeError("FailedBrowse"), Parm, *Error );
	

	// Open initial Viewport.
	if( Client )
	{
		// Init input.!!Temporary
		UInput::StaticInitInput();

		// Create viewport.
		UViewport* Viewport = GViewport = Client->NewViewport( NAME_None );

		// Create console.
		UClass* ConsoleClass = StaticLoadClass( UConsole::StaticClass(), NULL, TEXT("ini:Engine.Engine.Console"), NULL, LOAD_NoFail, NULL );
		Viewport->Console = ConstructObject<UConsole>( ConsoleClass );
		Viewport->Console->_Init( Viewport );

		// Spawn play actor.
		FString Error;
		if( !GLevel->SpawnPlayActor( Viewport, ROLE_SimulatedProxy, URL, Error ) )
			appErrorf( TEXT("%s"), *Error );
		Viewport->Input->Init( Viewport );
		Viewport->OpenWindow( 0, 0, INDEX_NONE, INDEX_NONE, INDEX_NONE, INDEX_NONE );
		GLevel->DetailChange( Viewport->RenDev->HighDetailActors );
		InitAudio();
		if( Audio )
			Audio->SetViewport( Viewport );
	}
	debugf( NAME_Init, TEXT("Game engine initialized") );
	
	// HP2: consume a command-line save slot before scheduling its travel so
	// later engine initialization cannot schedule the same load again.
	INT LoadGameSlot;
	if( appConsumeCommandLineLoadSlot(appCmdLine(),LoadGameSlot) )
	{
		GFileManager->MakeDirectory( *GSys->SavePath, 0 );
		GFileManager->MakeDirectory( *GSys->SaveSlotPath, 0 );
		TCHAR LoadURL[32];
		if( !appFormatLoadGameURL(LoadGameSlot,LoadURL,ARRAY_COUNT(LoadURL)) )
			appErrorf(TEXT("Invalid startup load slot %i"),LoadGameSlot);
		debugf( NAME_Log, TEXT("Loading game slot %i"), LoadGameSlot );
		GLevel->GetLevelInfo()->eventServerTravel(LoadURL, 0);
	}

	unguard;
}

//
// Pre exit.
//
void UGameEngine::Exit()
{
	guard(UGameEngine::Exit);
	Super::Exit();

	// Exit net.
	if( GLevel->NetDriver )
	{
		delete GLevel->NetDriver;
		GLevel->NetDriver = NULL;
	}

	unguard;
}

//
// Game exit.
//
void UGameEngine::Destroy()
{
	guard(UGameEngine::Destroy);

	// Game exit.
	if( GPendingLevel )
		CancelPending();
	GLevel = NULL;
	debugf( NAME_Exit, TEXT("Game engine shut down") );

	Super::Destroy();
	unguard;
}

//
// Progress text.
//
void UGameEngine::SetProgress( const TCHAR* Str1, const TCHAR* Str2, FLOAT Seconds )
{
	guard(UGameEngine::SetProgress);
	if( Client && Client->Viewports.Num() )
	{
		APlayerPawn* Actor = Client->Viewports(0)->Actor;
		if( Seconds==-1.f )
		{
			// Upgrade message.
			Actor->eventShowUpgradeMenu();
		}
		Actor->ProgressMessage[0] = Str1;
		Actor->ProgressColor[0].R = 255;
		Actor->ProgressColor[0].G = 255;
		Actor->ProgressColor[0].B = 255;

		Actor->ProgressMessage[1] = Str2;
		Actor->ProgressColor[1].R = 255;
		Actor->ProgressColor[1].G = 255;
		Actor->ProgressColor[1].B = 255;

		Actor->ProgressTimeOut    = Actor->Level->TimeSeconds + Seconds;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Command line executor.
-----------------------------------------------------------------------------*/

//
// This always going to be the last exec handler in the chain. It
// handles passing the command to all other global handlers.
//
UBOOL UGameEngine::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UGameEngine::Exec);
	const TCHAR* Str=Cmd;
	if( ParseCommand( &Str, TEXT("OPEN") ) )
	{
		FString Error;
		if( Client && Client->Viewports.Num() )
			SetClientTravel( Client->Viewports(0), Str, 0, TRAVEL_Partial );
		else
		if( !Browse( FURL(&LastURL,Str,TRAVEL_Partial), NULL, Error ) && Error!=TEXT("") )
			Ar.Logf( TEXT("Open failed: %s"), *Error );
		return 1;
	}
	else if( ParseCommand( &Str, TEXT("START") ) )
	{
		FString Error;
		if( Client && Client->Viewports.Num() )
			SetClientTravel( Client->Viewports(0), Str, 0, TRAVEL_Absolute );
		else
		if( !Browse( FURL(&LastURL,Str,TRAVEL_Absolute), NULL, Error ) && Error!=TEXT("") )
			Ar.Logf( TEXT("Start failed: %s"), *Error );
		return 1;
	}
	else if( ParseCommand( &Str, TEXT("SERVERTRAVEL") ) && (GIsServer && !GIsClient) )
	{
		GLevel->GetLevelInfo()->eventServerTravel(Str,0);
		return 1;
	}
	else if( (GIsServer && !GIsClient) && ParseCommand( &Str, TEXT("SAY") ) )
	{
		GLevel->GetLevelInfo()->eventBroadcastMessage(Str,1,NAME_None);
		return 1;
	}
	else if( ParseCommand(&Str, TEXT("DISCONNECT")) )
	{
		FString Error;
		if( Client && Client->Viewports.Num() )
		{
			if( GLevel && GLevel->NetDriver && GLevel->NetDriver->ServerConnection && GLevel->NetDriver->ServerConnection->Channels[0] )
			{
				GLevel->NetDriver->ServerConnection->Channels[0]->Close();
				GLevel->NetDriver->ServerConnection->FlushNet();
			}
			if( GPendingLevel && GPendingLevel->NetDriver && GPendingLevel->NetDriver->ServerConnection && GPendingLevel->NetDriver->ServerConnection->Channels[0] )
			{
				GPendingLevel->NetDriver->ServerConnection->Channels[0]->Close();
				GPendingLevel->NetDriver->ServerConnection->FlushNet();
			}
			SetClientTravel( Client->Viewports(0), TEXT("?failed"), 0, TRAVEL_Absolute );
		}
		else
		if( !Browse( FURL(&LastURL,TEXT("?failed"),TRAVEL_Absolute), NULL, Error ) && Error!=TEXT("") )
			Ar.Logf( TEXT("Disconnect failed: %s"), *Error );
		return 1;
	}
	else if( ParseCommand(&Str, TEXT("RECONNECT")) )
	{
		FString Error;
		if( Client && Client->Viewports.Num() )
			SetClientTravel( Client->Viewports(0), *LastURL.String(), 0, TRAVEL_Absolute );
		else
		if( !Browse( FURL(LastURL), NULL, Error ) && Error!=TEXT("") )
			Ar.Logf( TEXT("Reconnect failed: %s"), *Error );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("EXIT")) || ParseCommand(&Cmd,TEXT("QUIT")))
	{
		if( GLevel && GLevel->NetDriver && GLevel->NetDriver->ServerConnection && GLevel->NetDriver->ServerConnection->Channels[0] )
		{
			GLevel->NetDriver->ServerConnection->Channels[0]->Close();
			GLevel->NetDriver->ServerConnection->FlushNet();
		}
		if( GPendingLevel && GPendingLevel->NetDriver && GPendingLevel->NetDriver->ServerConnection && GPendingLevel->NetDriver->ServerConnection->Channels[0] )
		{
			GPendingLevel->NetDriver->ServerConnection->Channels[0]->Close();
			GPendingLevel->NetDriver->ServerConnection->FlushNet();
		}
		Ar.Log( TEXT("Closing by request") );
		appRequestExit( 0 );
		return 1;
	}
	else if( ParseCommand( &Str, TEXT("GETCURRENTTICKRATE") ) )
	{
		Ar.Logf( TEXT("%f"), CurrentTickRate );
		return 1;
	}
	else if( ParseCommand( &Str, TEXT("GETMAXTICKRATE") ) )
	{
		Ar.Logf( TEXT("%f"), GetMaxTickRate() );
		return 1;
	}
	else if( ParseCommand( &Str, TEXT("GSPYLITE") ) )
	{
		FString Error;
		appLaunchURL( TEXT("GSpyLite.exe"), TEXT(""), &Error );
		return 1;
	}
#if 1 // added by Legend on 4/12/2000
	else if( ParseCommand(&Str,TEXT("OBJCLEAN")) )
	{
		// quick hack to cleanup destroyed objects 
		if( GLevel )
			GLevel->CleanupDestroyed(1);
		return 1;
	}
#endif
	else if( ParseCommand(&Str,TEXT("SAVEGAME")) )
	{
		if( appIsDigit(Str[0]) )
		{
			GFileManager->MakeDirectory( *GSys->SavePath, 0 );
			GFileManager->MakeDirectory( *GSys->SaveSlotPath, 0 );
			
			
			//DEBUG
			TCHAR Filename[256];
			appSprintf( Filename, TEXT("%s") PATH_SEPARATOR TEXT("Save%i.usa"), *GSys->SaveSlotPath, appAtoi(Str) );
			debugf( NAME_Log, TEXT("Saving game...(before call) Filename: %s"), Filename );
			//
			SaveGame( appAtoi(Str) );
			
			//	The sounds are getting turned off during the save.  THis will restart them.
			//
			Exec(TEXT("RestartSounds"));
		}
		return 1;
	}
	else if( ParseCommand(&Str,TEXT("LOADGAME")) )
	{
		if( appIsDigit(Str[0]) )
		{
			if( !GLevel || GLevel->GetLevelInfo()->NextURL!=TEXT("") )
				return 1;

			// Whenever we try to load a game we should first load our persistentActorCache.
			LoadPersistentActorCache();

			const INT Slot = appAtoi(Str);
			GFileManager->MakeDirectory( *GSys->SavePath, 0 );
			GFileManager->MakeDirectory( *GSys->SaveSlotPath, 0 );
			debugf( NAME_Log, TEXT("Loading game slot %i"), Slot );
			TCHAR LoadURL[32];
			if( appFormatLoadGameURL(Slot,LoadURL,ARRAY_COUNT(LoadURL)) )
				GLevel->GetLevelInfo()->eventServerTravel(LoadURL,0);
		}
		return 1;
	}
	else if( ParseCommand(&Str,TEXT("SAVEPACTORS")) )
	{
		if( !GLevel )
			return 0;
		
		// Make sure we have the save directory

		GFileManager->MakeDirectory( *GSys->SavePath, 0 );
		GFileManager->MakeDirectory( *GSys->SaveSlotPath, 0 );
		
		// ********* Persistent Actor  *****************
		
		// Make sure to get the LevelEnterText for the name of the level 
		// (not URL->Map because it can be Save0.usa when loading a map)
				
		// Save our persistent actors
		GLevel->SavePersistentActors( GLevel->GetLevelInfo()->LevelEnterText );
		return 1;
		// *********************************************
	}
	else if( ParseCommand( &Cmd, TEXT("CANCEL") ) )
	{
		static UBOOL InCancel = 0;
		if( !InCancel )	
		{
			//!!Hack for broken Input subsystem.  JP.
			//!!Inside LoadMap(), ResetInput() is called,
			//!!which can retrigger an Exec call.
			InCancel = 1;
			if( GPendingLevel )
			{
				if( GPendingLevel->TrySkipFile() )
				{
					InCancel = 0;
					return 1;
				}
				SetProgress( LocalizeProgress("CancelledConnect"), TEXT(""), 2.f );
			}
			else
				SetProgress( TEXT(""), TEXT(""), 0.f );
			CancelPending();
			InCancel = 0;
		}
		return 1;
	}
	else if( GLevel && GLevel->Exec( Cmd, Ar ) )
	{
		return 1;
	}
	else if( GLevel && GLevel->GetLevelInfo()->Game && GLevel->GetLevelInfo()->Game->ScriptConsoleExec(Cmd,Ar,NULL) )
	{
		return 1;
	}
	else
	{
		// disallow set of pawn property if network game
		INT AllowSet = 1;
		if ( GLevel && (GLevel->GetLevelInfo()->NetMode == NM_Client) )
		{
			const TCHAR *Str = Cmd;
			if ( ParseCommand(&Str,TEXT("SET")) )
			{
				TCHAR ClassName[256];
				UClass* Class;
				if
				(	ParseToken( Str, ClassName, ARRAY_COUNT(ClassName), 1 )
				&&	(Class=FindObject<UClass>( ANY_PACKAGE, ClassName))!=NULL )
				{
					if ( Class->IsChildOf(AActor::StaticClass()) 
						&& !Class->IsChildOf(AGameInfo::StaticClass()) )
						AllowSet = 0;
				}
			}
		}
		if( AllowSet && UEngine::Exec( Cmd, Ar ) )
		{
			return 1;
		}
		else
			return 0;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Serialization.
-----------------------------------------------------------------------------*/

//
// Serializer.
//
void UGameEngine::Serialize( FArchive& Ar )
{
	guard(UGameEngine::Serialize);
	Super::Serialize( Ar );

	Ar << GLevel << GEntry << GPendingLevel;

	unguardobj;
}

/*-----------------------------------------------------------------------------
	Game entering.
-----------------------------------------------------------------------------*/

//
// Cancel pending level.
//
void UGameEngine::CancelPending()
{
	guard(UGameEngine::CancelPending);
	if( GPendingLevel )
	{
		if( GPendingLevel->NetDriver && GPendingLevel->NetDriver->ServerConnection && GPendingLevel->NetDriver->ServerConnection->Channels[0] )
		{
			GPendingLevel->NetDriver->ServerConnection->Channels[0]->Close();
			GPendingLevel->NetDriver->ServerConnection->FlushNet();
		}
		delete GPendingLevel;
		GPendingLevel = NULL;
	}
	unguard;
}

//
// Match Viewports to actors.
//
static void MatchViewportsToActors( UClient* Client, ULevel* Level, const FURL& URL )
{
	guard(MatchViewportsToActors);
	for( INT i=0; i<Client->Viewports.Num(); i++ )
	{
		FString Error;
		UViewport* Viewport = Client->Viewports(i);
		debugf( NAME_Log, TEXT("Spawning new actor for Viewport %s"), Viewport->GetName() );
		if( !Level->SpawnPlayActor( Viewport, ROLE_SimulatedProxy, URL, Error ) )
			appErrorf( TEXT("%s"), *Error );
	}
	unguardf(( TEXT("(%s)"), *Level->URL.Map ));
}

//
// Browse to a specified URL, relative to the current one.
//
UBOOL UGameEngine::Browse( FURL URL, const TMap<FString,FString>* TravelInfo, FString& Error )
{
	guard(UGameEngine::Browse);
	Error = TEXT("");
	const TCHAR* Option;

	// Convert .unreal link files.
	const TCHAR* LinkStr = TEXT(".unreal");//!!
	if( appStrstr(*URL.Map,LinkStr)-*URL.Map==appStrlen(*URL.Map)-appStrlen(LinkStr) )
	{
		debugf( TEXT("Link: %s"), *URL.Map );
		FString NewUrlString;
		if( GConfig->GetString( TEXT("Link")/*!!*/, TEXT("Server"), NewUrlString, *URL.Map ) )
		{
			// Go to link.
			URL = FURL( NULL, *NewUrlString, TRAVEL_Absolute );//!!
		}
		else
		{
			// Invalid link.
			guard(InvalidLink);
			Error = FString::Printf( LocalizeError("InvalidLink"), *URL.Map );
			unguard;
			return 0;
		}
	}

	// Crack the URL.
	debugf( TEXT("Browse: %s"), *URL.String() );

	// Handle it.
	if( !URL.Valid )
	{
		// Unknown URL.
		guard(UnknownURL);
		Error = FString::Printf( LocalizeError("InvalidUrl"), *URL.String() );
		unguard;
		return 0;
	}
	else if( URL.HasOption(TEXT("failed")) || URL.HasOption(TEXT("entry")) )
	{
		// Handle failure URL.
		guard(FailedURL);
		debugf( NAME_Log, LocalizeError("AbortToEntry") );
		if( GLevel && GLevel!=GEntry )
		{
			if( GLevel->BrushTracker )
			{
				delete GLevel->BrushTracker;
				GLevel->BrushTracker = NULL;
			}
			ResetLoaders( GLevel->GetOuter(), 1, 0 );
		}
		NotifyLevelChange();
		GLevel = GEntry;
		GLevel->GetLevelInfo()->LevelAction = LEVACT_None;
		check(Client && Client->Viewports.Num());
		MatchViewportsToActors( Client, GLevel, URL );
		if( Audio )
			Audio->SetViewport( Audio->GetViewport() );
		//CollectGarbage( RF_Native ); // Causes texture corruption unless you flush.
		if( URL.HasOption(TEXT("failed")) )
		{
			if( !GPendingLevel )
				SetProgress( LocalizeError("ConnectionFailed"), TEXT(""), 6.f );
		}
		unguard;
		return 1;
	}
	else if( URL.HasOption(TEXT("pop")) )
	{
		// Pop the hub.
		guard(PopURL);
		if( GLevel && GLevel->GetLevelInfo()->HubStackLevel>0 )
		{
			TCHAR Filename[256], FileToken[256], SavedPortal[256];
			appSprintf( Filename, TEXT("%s") PATH_SEPARATOR TEXT("Game%i.usa"), *GSys->SaveSlotPath, GLevel->GetLevelInfo()->HubStackLevel-1 );
			if( !appFilePathToFURLToken(Filename,FileToken,ARRAY_COUNT(FileToken)) )
				return 0;
			appStrcpy( SavedPortal, *URL.Portal );
			URL = FURL( &URL, FileToken, TRAVEL_Partial );
			URL.Portal = SavedPortal;
		}
		else return 0;
		unguard;
	}
	else if( URL.HasOption(TEXT("restart")) )
	{
		// Handle restarting.
		guard(RestartURL);
		URL = LastURL;
		unguard;
	}
	else if( (Option=URL.GetOption(TEXT("load="),NULL))!=NULL )
	{
		// Handle loadgame.
		guard(LoadURL);
		FPushMemTag Push(TEXT("LoadGame"));
		const INT Slot = appAtoi(Option);
		FString Error, Temp=FString::Printf( TEXT("%s") PATH_SEPARATOR TEXT("Save%i.usa"), *GSys->SaveSlotPath, Slot );
		FURL SaveURL(*Temp);
		SaveURL.AddOption(TEXT("load"));
		if( LoadMap(SaveURL,NULL,NULL,Error) )
		{
			// Copy the hub stack.
			INT i;
			for( i=0; i<GLevel->GetLevelInfo()->HubStackLevel; i++ )
			{
				TCHAR Src[256], Dest[256];//!!
				appSprintf( Src, TEXT("%s") PATH_SEPARATOR TEXT("Save%i%i.usa"), *GSys->SaveSlotPath, Slot, i );
				appSprintf( Dest, TEXT("%s") PATH_SEPARATOR TEXT("Game%i.usa"), *GSys->SaveSlotPath, i );
				GFileManager->Copy( Src, Dest );
			}
			while( 1 )
			{
				Temp = FString::Printf( TEXT("%s") PATH_SEPARATOR TEXT("Game%i.usa"), *GSys->SaveSlotPath, i++ );
				if( GFileManager->FileSize(*Temp)<=0 )
					break;
				GFileManager->Delete( *Temp );
			}
			if( GRepairSaveSlot==Slot && !GRepairSavePending )
				GRepairSavePending = 1;
			LastURL = GLevel->URL;
			return 1;
		}
		else return 0;
		unguard;
	}

	// Handle normal URL's.
	if( URL.IsLocalInternal() )
	{
		// Local map file.
		guard(LocalMapURL);
		return LoadMap( URL, NULL, TravelInfo, Error )!=NULL;
		unguard;
	}
	else if( URL.IsInternal() && GIsClient )
	{
		// Network URL.
		guard(NetworkURL);
		if( GPendingLevel )
			CancelPending();
		GPendingLevel = new UNetPendingLevel( this, URL );
		if( !GPendingLevel->NetDriver )
		{
			SetProgress( TEXT("Networking Failed"), *GPendingLevel->Error, 6.f );
			delete GPendingLevel;
			GPendingLevel = NULL;
		}
		return 0;
		unguard;
	}
	else if( URL.IsInternal() )
	{
		// Invalid.
		guard(InvalidURL);
		Error = LocalizeError("ServerOpen");
		unguard;
		return 0;
	}
	else
	{
		// External URL.
		guard(ExternalURL);
		appLaunchURL( *URL.String(), TEXT(""), &Error );
		unguard;
		return 0;
	}
	unguard;
}

//
// Notify that level is changing
//
void UGameEngine::NotifyLevelChange()
{
	guard(UGameEngine::NotifyLevelChange);
	if( Client && Client->Viewports.Num() && Client->Viewports(0)->Console )
		Client->Viewports(0)->Console->eventNotifyLevelChange();
	unguard;	
}

// Fixup a map
// hack to post release fix map actor problems without breaking compatibility
void UGameEngine::FixUpLevel( )
{
	if ( appStricmp(GLevel->GetFullName(), TEXT("Level CTF-Coret.MyLevel"))==0 )
	{
		debugf(TEXT("Fixing up CTF-Coret"));

		ANavigationPoint *Nav = GLevel->GetLevelInfo()->NavigationPointList;
		while (Nav)
		{
			if ( appStricmp(Nav->GetName(), TEXT("AlternatePath12"))==0
				|| appStricmp(Nav->GetName(), TEXT("AlternatePath13"))==0 )
			{
				debugf(NAME_Log, TEXT("Fixed up %s"),Nav->GetName());
				Nav->bTwoWay = 1;
			}
			else if ( appStricmp(Nav->GetName(), TEXT("PlayerStart5"))==0 )
			{
				APlayerStart *PS = Cast<APlayerStart>(Nav);
				if ( PS )
				{
					PS->bEnabled = false;
					debugf(NAME_Log, TEXT("Fixed up %s"),Nav->GetName());
				}
			}

			Nav = Nav->nextNavigationPoint;
		}
	}
	debugf(TEXT("Level is %s"), GLevel->GetFullName());

}
//
// Load a map.
//
ULevel* UGameEngine::LoadMap( const FURL& URL, UPendingLevel* Pending, const TMap<FString,FString>* TravelInfo, FString& Error )
{
	guard(UGameEngine::LoadMap);
	FPushMemTag Push(TEXT("LoadMap"));
	Error = TEXT("");

	if( GLevel )
		GMalloc->DumpAllocs();
	debugf( NAME_Log, TEXT("LoadMap: %s"), *URL.String() );
	FTime LoadTime[2] = { appSeconds(), appProcessSeconds() };
	appSetStartupRandTraceMap( *URL.Map, *URL.String() );
	GInitRunaway();

	// Remember current level's stack level.
	INT SavedHubStackLevel = GLevel ? GLevel->GetLevelInfo()->HubStackLevel : 0;

	// Display loading screen.
	guard(LoadingScreen);
	if( Client && Client->Viewports.Num() && GLevel )
	{
		GLevel->GetLevelInfo()->Pauser = TEXT("");
		APlayerPawn* PP = Client->Viewports(0)->Actor;
		if( PP )
			PP->bShowMenu = 0;

		// Fade out if needed.
		UViewport* Viewport=Client->Viewports(0);
		GLevel->GetLevelInfo()->LevelAction = LEVACT_None;

		if( Viewport->Console->FadeoutTime > 0.f && Viewport->Console->DrewWorld() )
		{
			FTime Prev = appSeconds();
			while( Viewport->Actor->FlashFog.W > 0.f )
			{
				Draw( Viewport );
				FTime Cur = appSeconds();
				Viewport->Actor->FlashFog.W -= (Cur-Prev) / Viewport->Console->FadeoutTime;
				Prev = Cur;
			}
		}

		Viewport->Actor->FlashFog.W = 0.f;
		Draw( Viewport );

		if( Audio )
			Audio->SetViewport( Audio->GetViewport() );
	}
	unguard;

	// Get network package map.
	UPackageMap* PackageMap = NULL;
	if( Pending )
		PackageMap = Pending->GetDriver()->ServerConnection->PackageMap;

	// Verify that we can load all packages we need.
	UObject* MapParent = NULL;
	guard(VerifyPackages);
	try
	{
		BeginLoad();
		if( Pending )
		{
			// Verify that we can load everything needed for client in this network level.
			for( INT i=0; i<PackageMap->List.Num(); i++ )
				PackageMap->List(i).Linker = GetPackageLinker
				(
					PackageMap->List(i).Parent,
					NULL,
					LOAD_Verify | LOAD_Throw | LOAD_NoWarn | LOAD_NoVerify,
					NULL,
					&PackageMap->List(i).Guid
				);
			for( INT i=0; i<PackageMap->List.Num(); i++ )
				VerifyLinker( PackageMap->List(i).Linker );
			if( PackageMap->List.Num() )
				MapParent = PackageMap->List(0).Parent;
		}
		LoadObject<ULevel>( MapParent, TEXT("MyLevel"), *URL.Map, LOAD_Verify | LOAD_Throw | LOAD_NoWarn, NULL );
		EndLoad();

#if HPDEMO
		if( !Pending || !Pending->DemoRecDriver )
		{
			FString FileName(FString(TEXT("../Maps/"))+URL.Map);
			if( FileName.Right(4).Caps() != TEXT(".UNR"))
				FileName = FileName + TEXT(".unr");
			INT FileSize = GFileManager->FileSize( *FileName );
			debugf(TEXT("Looking for file: %s %d"), *FileName, FileSize);
			if( //FileSize != 0 &&
				( FileName.Caps() != TEXT("../MAPS/DEMO_PC.UNR")	) &&
				( FileName.Caps() != TEXT("../MAPS/ENTRY.UNR") )&&
				( FileName.Caps() != TEXT("../MAPS/STARTUP.UNR") )
				)
			{
				Error = TEXT("Sorry, only the retail version of UT can load third party maps.");
				SetProgress( LocalizeError(TEXT("UrlFailed"),TEXT("Core")), *Error, 6.f );
				return NULL;
			}
		}
#endif


#if DEMOVERSION
		// If we area demo, prevent third party maps from being loaded.
		if( !Pending || !Pending->DemoRecDriver )
		{
			FString FileName(FString(TEXT("../Maps/"))+URL.Map);
			if( FileName.Right(4).Caps() != TEXT(".UNR"))
				FileName = FileName + TEXT(".unr");
			INT FileSize = GFileManager->FileSize( *FileName );
			debugf(TEXT("Looking for file: %s %d"), *FileName, FileSize);
			if( //FileSize != 0 &&
				( FileName.Caps() != TEXT("../MAPS/DM-TURBINEDEMO.UNR")	|| FileSize != 2135105 ) &&
				( FileName.Caps() != TEXT("../MAPS/DM-PHOBOSDEMO.UNR")	|| FileSize != 1618994 ) &&
				( FileName.Caps() != TEXT("../MAPS/DM-MORPHEUSDEMO.UNR")|| FileSize != 1193759 ) &&
				( FileName.Caps() != TEXT("../MAPS/DM-TEMPESTDEMO.UNR")	|| FileSize != 2152238 ) &&
				( FileName.Caps() != TEXT("../MAPS/CTF-CORETDEMO.UNR")	|| FileSize != 3498978 ) &&
				( FileName.Caps() != TEXT("../MAPS/DOM-SESMARDEMO.UNR")	|| FileSize != 2155658 ) &&
				( FileName.Caps() != TEXT("../MAPS/ENTRY.UNR")			|| FileSize != 34822 ) &&
				( FileName.Caps() != TEXT("../MAPS/UT-LOGO-MAP.UNR")	|| FileSize != 34884 ) )
			{
				Error = TEXT("Sorry, only the retail version of UT can load third party maps.");
				SetProgress( LocalizeError(TEXT("UrlFailed"),TEXT("Core")), *Error, 6.f );
				return NULL;
			}
		}
#endif
	}
	#if _MSC_VER
	catch( TCHAR* CatchError )
	#else
	catch( char* CatchError )
	#endif
	{
		// Safely failed loading.
		EndLoad();
		#if _MSC_VER
			Error = CatchError;
		#else
			Error = appFromAnsi( CatchError );
		#endif
		SetProgress( LocalizeError(TEXT("UrlFailed"),TEXT("Core")), *Error, 6.f );

		if( Client && Client->Viewports.Num()>0 )
		{
			// Restore visibility.
			UViewport* Viewport=Client->Viewports(0);
			GLevel->GetLevelInfo()->LevelAction = LEVACT_None;
			Viewport->Actor->FlashFog.W = 1.f;
			Viewport->Actor->ConstantGlowFog.W = 0.f;
			Viewport->Actor->FadeRate = 0.f;
		}
		return NULL;
	}
	unguard;

	// Notify of the level change, before we dissociate Viewport actors
	guard(NotifyLevelChange);
	if( GLevel )
		NotifyLevelChange();
	unguard;

	// Dissociate Viewport actors.
	guard(DissociateViewports);
	if( Client )
	{
		for( INT i=0; i<Client->Viewports.Num(); i++ )
		{
			APlayerPawn* Actor          = Client->Viewports(i)->Actor;
			ULevel*      Level          = Actor->GetLevel();
			Actor->Player               = NULL;
			Client->Viewports(i)->Actor = NULL;
			Level->DestroyActor( Actor );
		}
	}
	unguard;

	// Clean up game state.
	guard(ExitLevel);
	if( GLevel )
	{
		// Shut down.
		ResetLoaders( GLevel->GetOuter(), 0, 1 );
		if( GLevel->BrushTracker )
		{
			delete GLevel->BrushTracker;
			GLevel->BrushTracker = NULL;
		}
		if( GLevel->NetDriver )
		{
			delete GLevel->NetDriver;
			GLevel->NetDriver = NULL;
		}
		if( GLevel->DemoRecDriver )
		{
			delete GLevel->DemoRecDriver;
			GLevel->DemoRecDriver = NULL;
		}
		if( URL.HasOption(TEXT("push")) )
		{
			// Save the current level minus players actors.
			GLevel->CleanupDestroyed( 1 );
			TCHAR Filename[256];
			appSprintf( Filename, TEXT("%s") PATH_SEPARATOR TEXT("Game%i.usa"), *GSys->SaveSlotPath, SavedHubStackLevel );
			SavePackage( GLevel->GetOuter(), GLevel, 0, Filename, GLog );
		}
		GLevel = NULL;
	}
	unguard;

	// Load the level and all objects under it, using the proper Guid.
	ALevelInfo* Info = LoadObject<ALevelInfo>( MapParent, TEXT("LevelInfo0"), *URL.Map, LOAD_NoFail, NULL );
	
	if( Info )
	{
		// Save the original map name in LevelInfo, so that it's retrievable from save games.
		if( Info->LevelEnterText == TEXT("") )
			Info->LevelEnterText = URL.Map;
		if( Client && Client->Viewports.Num() )
		{
			// Draw level info.
			UViewport* Viewport = Client->Viewports(0);
			DWORD LockFlags=0;
			if( Viewport->Lock(FPlane(0.f),FPlane(0.f),FPlane(0.f),LockFlags,NULL,NULL) )
			{
				Viewport->RenDev->EndFlash();
				if( Viewport->Console && Viewport->Canvas )
				{
					// Let console display any level info.
					Viewport->Console->eventDrawLevelInfo( Viewport->Canvas, Info->LevelEnterText );
				}
				Viewport->Unlock( 1 );
			}
		}
	}

	{
		FAppRandTracePhaseScope StartupPhaseTrace( "pre_load_map" );
	guard(LoadLevel);
	GLevel = LoadObject<ULevel>( MapParent, TEXT("MyLevel"), *URL.Map, LOAD_NoFail, NULL );
	unguard;
	}
	FAppRandTracePhaseScope StartupPhaseTrace( "post_load_map" );
	if( HP2ActorSlotDumpPostDeserialize )
		HP2ActorSlotDumpPostDeserialize( GLevel );
	
	// If pending network level.
	if( Pending )
	{
		// If playing this network level alone, ditch the pending level.
		if( Pending && Pending->LonePlayer )
			Pending = NULL;
		
		// Setup network package info.
		PackageMap->Compute();
		for( INT i=0; i<PackageMap->List.Num(); i++ )
			if( PackageMap->List(i).LocalGeneration!=PackageMap->List(i).RemoteGeneration )
				Pending->GetDriver()->ServerConnection->Logf( TEXT("HAVE GUID=%s GEN=%i"), PackageMap->List(i).Guid.String(), PackageMap->List(i).LocalGeneration );
	}

	// Verify classes.
	guard(VerifyClasses);
	VERIFY_CLASS_OFFSET( A, Actor,       Owner         );
	VERIFY_CLASS_OFFSET( A, Actor,       TimerCounter  );
	VERIFY_CLASS_OFFSET( A, PlayerPawn,  Player        );
	VERIFY_CLASS_OFFSET( A, PlayerPawn,  MaxStepHeight );
	unguard;

	// Get LevelInfo.
	check(GLevel);
	Info = GLevel->GetLevelInfo();
	Info->ComputerName = appComputerName();

	// Handle pushing.
	guard(ProcessHubStack);
	Info->HubStackLevel
	=	URL.HasOption(TEXT("load")) ? Info->HubStackLevel
	:	URL.HasOption(TEXT("push")) ? SavedHubStackLevel+1
	:	URL.HasOption(TEXT("pop" )) ? Max(SavedHubStackLevel-1,0)
	:	URL.HasOption(TEXT("peer")) ? SavedHubStackLevel
	:	                              0;
	unguard;

	// Handle pending level.
	guard(ActivatePending);
	if( Pending )
	{
		check(Pending==GPendingLevel);

		// Hook network driver up to level.
		GLevel->NetDriver = Pending->NetDriver;
		if( GLevel->NetDriver )
			GLevel->NetDriver->Notify = GLevel;

		// Hook demo playback driver to level
		GLevel->DemoRecDriver = Pending->DemoRecDriver;
		if( GLevel->DemoRecDriver )
			GLevel->DemoRecDriver->Notify = GLevel;

		// Setup level.
		GLevel->GetLevelInfo()->NetMode = NM_Client;
	}
	else check(!GLevel->NetDriver);
	unguard;

	// ********* Persistent Actor Support ***********
	if( appStricmp(GLevel->GetOuter()->GetName(),TEXT("Entry"))!=0 )
	{
		// load the persistent actors
		guard(LoadPersistentActors);
		GLevel->LoadPersistentActors( URL.Map );
		unguard;
	}
	// *********************************************

	// Set level info.
	guard(InitLevel);
	if( !URL.GetOption(TEXT("load"),NULL) )
		GLevel->URL = URL;
	Info->EngineVersion = FString::Printf( TEXT("%i"), ENGINE_VERSION );
	Info->MinNetVersion = FString::Printf( TEXT("%i"), ENGINE_MIN_NET_VERSION );
	GLevel->Engine = this;
	if( TravelInfo )
		GLevel->TravelInfo = *TravelInfo;
	unguard;

	// Purge unused objects and flush caches.
	guard(Cleanup);
	if( appStricmp(GLevel->GetOuter()->GetName(),TEXT("Entry"))!=0 )
	{
		Flush(0);
		{for( TObjectIterator<AActor> It; It; ++It )
			if( It->IsIn(GLevel->GetOuter()) )
				It->SetFlags( RF_EliminateObject );}
		{for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) )
				GLevel->Actors(i)->ClearFlags( RF_EliminateObject );}
		CollectGarbage( RF_Native );
	}
	unguard;

	// Tell the audio driver to clean up.
	if( Audio )
		Audio->CleanUp();

	// Init collision.
	GLevel->SetActorCollision( 1 );

	// Setup zone distance table for sound damping. Fast enough: Approx 3 msec.
	guard(SetupZoneTable);
	QWORD OldConvConn[64];
	QWORD ConvConn[64];
	for( INT i=0; i<64; i++ )
	{
		for( INT j=0; j<64; j++ )
		{
			OldConvConn[i] = GLevel->Model->Zones[i].Connectivity;
			if( i == j )
				GLevel->ZoneDist[i][j] = 0;
			else
				GLevel->ZoneDist[i][j] = 255;
		}
	}
	for( INT i=1; i<64; i++ )
	{
		for( INT j=0; j<64; j++ )
			for( INT k=0; k<64; k++ )
				if( (GLevel->ZoneDist[j][k] > i) && ((OldConvConn[j] & ((QWORD)1 << k)) != 0) )
					GLevel->ZoneDist[j][k] = i;
		for( INT j=0; j<64; j++ )
			ConvConn[j] = 0;
		for( INT j=0; j<64; j++ )
			for( INT k=0; k<64; k++ )
				if( (OldConvConn[j] & ((QWORD)1 << k)) != 0 )
					ConvConn[j] = ConvConn[j] | OldConvConn[k];
		for( INT j=0; j<64; j++ )
			OldConvConn[j] = ConvConn[j];
	}
	unguard;

	// Update the LevelInfo's time.
	GLevel->UpdateTime(Info);

	// Init the game info.
	TCHAR Options[1024]=TEXT("");
	TCHAR GameClassName[256]=TEXT("");
	FString Error=TEXT("");
	guard(InitGameInfo);
	for( INT i=0; i<URL.Op.Num(); i++ )
	{
		appStrcat( Options, TEXT("?") );
		appStrcat( Options, *URL.Op(i) );
		Parse( *URL.Op(i), TEXT("GAME="), GameClassName, ARRAY_COUNT(GameClassName) );
	}
	if( GLevel->IsServer() && !Info->Game )
	{
		// Get the GameInfo class.
		UClass* GameClass=NULL;
		if( !GameClassName[0] )
		{
			GameClass=Info->DefaultGameType;
			if( !GameClass )
				GameClass = StaticLoadClass( AGameInfo::StaticClass(), NULL, Client ? TEXT("ini:Engine.Engine.DefaultGame") : TEXT("ini:Engine.Engine.DefaultServerGame"), NULL, LOAD_NoFail, PackageMap );
		}
		else GameClass = StaticLoadClass( AGameInfo::StaticClass(), NULL, GameClassName, NULL, LOAD_NoFail, PackageMap );

		// Spawn the GameInfo.
		debugf( NAME_Log, TEXT("Game class is '%s'"), GameClass->GetName() );
		Info->Game = (AGameInfo*)GLevel->SpawnActor( GameClass );
		check(Info->Game!=NULL);
	}
	unguard;

	// Listen for clients.
	guard(Listen);
	if( !Client || URL.HasOption(TEXT("Listen")) )
	{
		if( GPendingLevel )
		{
			guard(CancelPendingForListen);
			check(!Pending);
			delete GPendingLevel;
			GPendingLevel = NULL;
			unguard;
		}
		FString Error;
		if( !GLevel->Listen( Error ) )
			appErrorf( LocalizeError("ServerListen"), *Error );
	}
	unguard;

	// Init detail.
	Info->bHighDetailMode = 1;
	if
	(	Client
	&&	Client->Viewports.Num()
	&&	Client->Viewports(0)->RenDev
	&&	!Client->Viewports(0)->RenDev->HighDetailActors )
		Info->bHighDetailMode = 0;

	// Init level gameplay info.
	guard(BeginPlay);
	GLevel->iFirstDynamicActor = 0;
	if( !Info->bBegunPlay )
	{
		// fix up level problems
		FixUpLevel();

		// Lock the level.
		debugf( NAME_Log, TEXT("Bringing %s up for play (%i)..."), GLevel->GetFullName(), appRound(GetMaxTickRate()) );
		GLevel->TimeSeconds = 0.f;
		GLevel->GetLevelInfo()->TimeSeconds = 0;

		// Init touching actors.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) )
				for( INT j=0; j<ARRAY_COUNT(GLevel->Actors(i)->Touching); j++ )
					GLevel->Actors(i)->Touching[j] = NULL;

		// Kill off actors that aren't interesting to the client.
		if( !GLevel->IsServer() )
		{
			for( INT i=0; i<GLevel->Actors.Num(); i++ )
			{
				AActor* Actor = GLevel->Actors(i);
				if( Actor )
				{
					if( Actor->bStatic || Actor->bNoDelete )
						Exchange( Actor->Role, Actor->RemoteRole );
					else
						GLevel->DestroyActor( Actor );
				}
			}
		}
		
		// Init scripting.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) )
				GLevel->Actors(i)->InitExecution();

		// Enable actor script calls.
		Info->bBegunPlay = 1;
		Info->bStartup = 1;
		
		// Init the game.
		if( Info->Game )
			Info->Game->eventInitGame( Options, Error );

		// Send PreBeginPlay.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) && !GLevel->Actors(i)->bScriptInitialized )
			{
				HP2ActorTransitionPreBeginBefore( GLevel->Actors(i) );
				GLevel->Actors(i)->eventPreBeginPlay();
				HP2ActorTransitionPreBeginAfter( GLevel->Actors(i) );
			}

		// Set BeginPlay.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) && !GLevel->Actors(i)->bScriptInitialized )
				GLevel->Actors(i)->eventBeginPlay();

		// Set zones.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) && !GLevel->Actors(i)->bScriptInitialized )
				GLevel->SetActorZone( GLevel->Actors(i), 1, 1 );

		// Post begin play.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) && !GLevel->Actors(i)->bScriptInitialized )
				GLevel->Actors(i)->eventPostBeginPlay();

		// Begin scripting.
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
			if( GLevel->Actors(i) && !GLevel->Actors(i)->bScriptInitialized )
				GLevel->Actors(i)->eventSetInitialState();

		// Find bases
		for( INT i=0; i<GLevel->Actors.Num(); i++ )
		{
			if( GLevel->Actors(i) ) 
			{
				if ( GLevel->Actors(i)->AttachTag != NAME_None )
				{
					//find actor to attach self onto
					for( INT j=0; j<GLevel->Actors.Num(); j++ )
					{
						if( GLevel->Actors(j) && (GLevel->Actors(j)->Tag == GLevel->Actors(i)->AttachTag) )
						{
							GLevel->Actors(i)->SetBase(GLevel->Actors(j), 0);
							break;
						}
					}
				}
				else if( !GLevel->Actors(i)->Base && GLevel->Actors(i)->bCollideWorld 
				 && (GLevel->Actors(i)->IsA(ADecoration::StaticClass()) || GLevel->Actors(i)->IsA(AInventory::StaticClass()) || GLevel->Actors(i)->IsA(APawn::StaticClass())) 
				 &&	((GLevel->Actors(i)->Physics == PHYS_None) || (GLevel->Actors(i)->Physics == PHYS_Rotating)) )
				{
					 GLevel->Actors(i)->FindBase();
					 if ( GLevel->Actors(i)->Base == Info )
						 GLevel->Actors(i)->SetBase(NULL, 0);
				}
			}
		}
		Info->bStartup = 0;
	}
	else GLevel->TimeSeconds = GLevel->GetLevelInfo()->TimeSeconds;
	appSetStartupRandTraceLifecycleBoundary( "bring_up_for_play_complete" );
	unguard;

	// Rearrange actors: static first, then others.
	guard(Rearrange);
	TArray<AActor*> Actors;
	Actors.AddItem(GLevel->Actors(0));
	Actors.AddItem(GLevel->Actors(1));
	for( INT i=2; i<GLevel->Actors.Num(); i++ )
		if( GLevel->Actors(i) && GLevel->Actors(i)->bStatic && !GLevel->Actors(i)->bAlwaysRelevant )
			Actors.AddItem( GLevel->Actors(i) );
	GLevel->iFirstNetRelevantActor=Actors.Num();
	for( INT i=2; i<GLevel->Actors.Num(); i++ )
		if( GLevel->Actors(i) && GLevel->Actors(i)->bStatic && GLevel->Actors(i)->bAlwaysRelevant )
			Actors.AddItem( GLevel->Actors(i) );
	GLevel->iFirstDynamicActor=Actors.Num();
	for( INT i=2; i<GLevel->Actors.Num(); i++ )
		if( GLevel->Actors(i) && !GLevel->Actors(i)->bStatic )
			Actors.AddItem( GLevel->Actors(i) );
	GLevel->Actors.Empty();
	GLevel->Actors.Add( Actors.Num() );
	for( INT i=0; i<Actors.Num(); i++ )
		GLevel->Actors(i) = Actors(i);
	unguard;
	
	
	
	// Cleanup profiling.
#if DO_GUARD_SLOW
	guard(CleanupProfiling);
	for( TObjectIterator<UFunction> It; It; ++It )
		It->Calls = It->Cycles=0;
	GTicks=1;
	unguard;
#endif


	// Client init.
	guard(ClientInit);
	if( Client )
	{
		// Match Viewports to actors.
		MatchViewportsToActors( Client, GLevel->IsServer() ? GLevel : GEntry, URL );

		// Init brush tracker.
		if( appStricmp(GLevel->GetOuter()->GetName(),TEXT("Entry"))!=0 )//!!
			GLevel->BrushTracker = GNewBrushTracker( GLevel );

		// Set up audio.
		if( Audio )
			Audio->SetViewport( Audio->GetViewport() );

		// Reset viewports.
		for( INT i=0; i<Client->Viewports.Num(); i++ )
		{
			UViewport* Viewport = Client->Viewports(i);
			Viewport->Input->ResetInput();
			if( Viewport->RenDev )
				Viewport->RenDev->Flush(1);
		}
	}
	unguard;
	


	// ********** GameState Support ****************
	if( appStricmp(GLevel->GetOuter()->GetName(),TEXT("Entry"))!=0 )
	{
		// Check gameState compared to the actor list
		// This needs to be done *AFTER* the Client init 
		// so that the gamestate has a chance to travel.
		guard(ScreenActorsByGameState);
		GLevel->ScreenActorsByGameState();
		unguard;
	}
	// *********************************************

	// Init detail.
	GLevel->DetailChange( Info->bHighDetailMode );
	
	
	
	// Remember the URL.
	guard(RememberURL);
	LastURL = URL;
	unguard;

	// Remember DefaultPlayer options.
	if( GIsClient )
	{
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("Name" ), TEXT("User") );
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("Team" ), TEXT("User") );
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("Class"), TEXT("User") );
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("Skin" ), TEXT("User") );
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("Face" ), TEXT("User") );
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("Voice" ), TEXT("User") );
		URL.SaveURLConfig( TEXT("DefaultPlayer"), TEXT("OverrideClass" ), TEXT("User") );
	}
	
	debugf( TEXT("Load time %s: %f seconds total, %f app"), 
		*URL.Map, appSeconds()-LoadTime[0], appProcessSeconds()-LoadTime[1] );

	// Successfully started local level.
	return GLevel;
	unguard;
}

/*-----------------------------------------------------------------------------
	Game Viewport functions.
-----------------------------------------------------------------------------*/

FLOAT UGameEngine::GetConsoleUIScale( FLOAT Width, FLOAT Height, FLOAT UserScale )
{
	if( Width<=0.f || Height<=0.f )
		return 1.f;
	if( appIsNan(UserScale) )
		UserScale = 1.f;
	UserScale = Clamp(UserScale,0.75f,2.f);
	return Min(Width/640.f,Height/480.f) * UserScale;
}

static void ApplyConsoleUIScale( UViewport* Viewport )
{
	if( !Viewport || !Viewport->Console )
		return;

	UConsole* Console = Viewport->Console;
	static UClass* CachedConsoleClass = NULL;
	static UObjectProperty* RootProperty = NULL;
	if( CachedConsoleClass != Console->GetClass() )
	{
		CachedConsoleClass = Console->GetClass();
		RootProperty = FindField<UObjectProperty>( CachedConsoleClass, TEXT("Root") );
	}
	if( !RootProperty )
		return;

	UObject* Root = *(UObject**)((BYTE*)Console + RootProperty->Offset);
	if( !Root )
		return;

	static UClass* CachedRootClass = NULL;
	static UFloatProperty* GUIScaleProperty = NULL;
	static UFloatProperty* RealWidthProperty = NULL;
	static UFloatProperty* RealHeightProperty = NULL;
	if( CachedRootClass != Root->GetClass() )
	{
		CachedRootClass = Root->GetClass();
		GUIScaleProperty = FindField<UFloatProperty>( CachedRootClass, TEXT("GUIScale") );
		RealWidthProperty = FindField<UFloatProperty>( CachedRootClass, TEXT("RealWidth") );
		RealHeightProperty = FindField<UFloatProperty>( CachedRootClass, TEXT("RealHeight") );
	}
	if( !GUIScaleProperty || !RealWidthProperty || !RealHeightProperty )
		return;

	const FLOAT RealWidth = *(FLOAT*)((BYTE*)Root + RealWidthProperty->Offset);
	const FLOAT RealHeight = *(FLOAT*)((BYTE*)Root + RealHeightProperty->Offset);
	const FLOAT DesiredScale = UGameEngine::GetConsoleUIScale(
		RealWidth,RealHeight,Viewport->GetOuterUClient()->GetUIScale());
	const FLOAT CurrentScale = *(FLOAT*)((BYTE*)Root + GUIScaleProperty->Offset);
	if( Abs(CurrentScale - DesiredScale) <= 0.0001f )
		return;

	UFunction* SetScale = Root->FindFunction( TEXT("SetScale") );
	if( SetScale )
	{
		struct { FLOAT NewScale; } Parms;
		Parms.NewScale = DesiredScale;
		Root->ProcessEvent( SetScale, &Parms );
	}
}

//
// Draw a global view.
//
void UGameEngine::Draw( UViewport* Viewport, UBOOL Blit, BYTE* HitData, INT* HitSize )
{
	guard(UGameEngine::Draw);

	// If not up and running yet, don't draw.
	if( !GIsRunning || GIsDrawing )
		return;
	GIsDrawing = 1;
	FPushMemTag Push(TEXT("UGameEngine::Draw"));
	UpdateConnectingMessage();

	// Get view location.
	AActor*      ViewActor    = Viewport->Actor;
	FVector      ViewLocation = ViewActor->Location;
	FRotator     ViewRotation = ViewActor->Rotation;
	Viewport->Actor->eventPlayerCalcView( ViewActor, ViewLocation, ViewRotation );
	check(ViewActor);

	if( Viewport->RenDev->PrecacheOnFlip && !Viewport->bSuspendPrecaching && ViewActor->Level->LevelAction == LEVACT_None )
	{
		Viewport->RenDev->PrecacheOnFlip = 0;
		if ( !ViewActor->Level->bNeverPrecache )
			Render->Precache( Viewport );
	}

	// See if viewer is inside world.
	DWORD LockFlags=0;
	FCheckResult Hit;
	if( !GLevel->Model->PointCheck(Hit,NULL,ViewLocation,FVector(0,0,0),0) )
		LockFlags |= LOCKR_ClearScreen;

#if defined(LEGEND)
	if( Viewport->Actor->IsA( APlayerPawn::StaticClass() ) )
	{
		// call the PlayerPawn Render Control Interface (RCI) to assess clear-screen operations
		if( Viewport->Actor->ClearScreen() )
		{
			LockFlags |= LOCKR_ClearScreen;
		}

		// call the PlayerPawn Render Control Interface (RCI) to assess lighting recomputation
		//
		// WARNING: RecomputeLighting() should *not* return false regularly, or rendering 
		//          performance will be severly compromised
		if( Viewport->Actor->RecomputeLighting() )
		{
			guard(RecomputeLighting);
			Flush();
			unguard;
		}
	}
#endif

	// Lock the Viewport.
	check(Render);
	FPlane FlashScale = FPlane( Client->ScreenFlashes ? Clamp( 0.5f*Viewport->Actor->FlashFog.W, 0.f, 1.f ) : 0.5f );
	FPlane FlashFog   = Client->ScreenFlashes ? Viewport->Actor->FlashFog : FVector(0,0,0);
	FlashFog.X   = Clamp( FlashFog.X  , 0.f, 1.f );
	FlashFog.Y   = Clamp( FlashFog.Y  , 0.f, 1.f );
	FlashFog.Z   = Clamp( FlashFog.Z  , 0.f, 1.f );
	FlashFog.W   = 0.f;
	if( Viewport->Lock(FlashScale,FlashFog,FPlane(0,0,0,0),LockFlags,HitData,HitSize) )
	{
		// Setup rendering coords.
		FSceneNode* Frame = Render->CreateMasterFrame( Viewport, ViewLocation, ViewRotation, NULL );

		// Update level audio.
		if( Audio )
		{
			clock(GLevel->AudioTickCycles);
			Audio->Update( Frame );
			unclock(GLevel->AudioTickCycles);
		}

		// Render.
		Render->PreRender( Frame );
		Viewport->Canvas->Render = Render;
		if( Viewport->Console )
			Viewport->Console->PreRender( Frame );
		Viewport->Canvas->Update( Frame );
		Viewport->Actor->eventPreRender( Viewport->Canvas );
		if( Frame->X>0 && Frame->Y>0 && (!Viewport->Console || Viewport->Console->GetDrawWorld()) )
			Render->DrawWorld( Frame );
		Viewport->RenDev->EndFlash();
		Viewport->RenDev->BeginUI( Frame );
		Viewport->Actor->eventPostRender( Viewport->Canvas );
		if( Viewport->Console )
		{
			UCanvas* Canvas = Viewport->Canvas;
			const FLOAT SavedOrgX = Canvas->OrgX;
			const FLOAT SavedOrgY = Canvas->OrgY;
			const FLOAT SavedClipX = Canvas->ClipX;
			const FLOAT SavedClipY = Canvas->ClipY;
			const FLOAT SavedMouseX = Viewport->WindowsMouseX;
			const INT SavedSizeX = Canvas->X;
			const INT SavedSizeY = Canvas->Y;
			const FLOAT SavedMouseY = Viewport->WindowsMouseY;
			const FLOAT Scale = UGameEngine::GetConsoleUIScale(
				SavedClipX,SavedClipY,Viewport->GetOuterUClient()->GetUIScale());
			const FLOAT UIWidth = Min(SavedClipX,640.f*Scale);
			const FLOAT UIHeight = Min(SavedClipY,480.f*Scale);
			const FLOAT OffsetX = Max(0.f,(SavedClipX-UIWidth)*0.5f);
			const FLOAT OffsetY = Max(0.f,(SavedClipY-UIHeight)*0.5f);
			Canvas->OrgX += OffsetX;
			Canvas->OrgY += OffsetY;
			Canvas->ClipX = UIWidth;
			Canvas->X = appRound(UIWidth);
			Canvas->Y = appRound(UIHeight);
			Canvas->ClipY = UIHeight;
			Viewport->WindowsMouseX -= OffsetX;
			Viewport->WindowsMouseY -= OffsetY;
			ApplyConsoleUIScale( Viewport );
			Viewport->Console->PostRender( Frame );
			Viewport->Console->eventPostRender( Canvas );
			Canvas->OrgX = SavedOrgX;
			Canvas->OrgY = SavedOrgY;
			Canvas->ClipX = SavedClipX;
			Canvas->ClipY = SavedClipY;
			Canvas->X = SavedSizeX;
			Canvas->Y = SavedSizeY;
			Viewport->WindowsMouseX = SavedMouseX;
			Viewport->WindowsMouseY = SavedMouseY;
		}
		if( Replay.Replaying() || Replay.Recording() )
		{
			// Show replay message.
			UCanvas* Canvas = Frame->Viewport->Canvas;
			Canvas->Color = FColor(255,255,255);
			Canvas->CurX=0;
			Canvas->CurY=Canvas->ClipY-40;
			Canvas->WrappedPrintf( Canvas->SmallFont, 0, TEXT("%s F=%d T=%.3f"), 
				Replay.Recording() ? TEXT("Recording") : TEXT("Replaying"),
				Replay.GetFrameNum(), GLevel->TimeSeconds.GetFloat() );
		}
		if( Audio )
			Audio->PostRender( Frame );

#if 0
/* BEGIN BETA VERSION */
		if(GLevel && GLevel->GetLevelInfo() && GLevel->GetLevelInfo()->Game && FString(GLevel->GetLevelInfo()->Game->GetClass()->GetName()) == FString(TEXT("UTIntro")))
		{
			if ( ((AGameInfo*) AGameInfo::StaticClass()->GetDefaultObject())->DemoBuild == 0 )
			{
				// "BETA VERSION" XOR'd with BetaDecoder
				static TCHAR BetaCypher[] = { 67, 4, 50, 41, 108, 125, 82, 27, 46, 55, 121, 25 };
				static TCHAR BetaDecoder[] = { 1, 65, 102, 104, 76, 43, 23, 73, 125, 126, 54, 87, 33, 78, 0 };
				static TCHAR BetaDecoded[] = TEXT("            "); // gets replaced with "BETA VERSION"

				for(INT i=0; BetaDecoded[i]; i++)
						BetaDecoded[i] = BetaCypher[i] ^ BetaDecoder[i];
			
				Frame->Viewport->Canvas->Color = FColor(255,255,255);
				Frame->Viewport->Canvas->CurX=0;
				Frame->Viewport->Canvas->CurY=0;
				Frame->Viewport->Canvas->WrappedPrintf( Frame->Viewport->Canvas->SmallFont, 0, BetaDecoded );
				Frame->Viewport->Canvas->CurX=Frame->Viewport->Canvas->ClipX - 72;
				Frame->Viewport->Canvas->CurY=0;
				Frame->Viewport->Canvas->WrappedPrintf( Frame->Viewport->Canvas->SmallFont, 0, BetaDecoded );
				Frame->Viewport->Canvas->CurX=0;
				Frame->Viewport->Canvas->CurY=Frame->Viewport->Canvas->ClipY - 10;
				Frame->Viewport->Canvas->WrappedPrintf( Frame->Viewport->Canvas->SmallFont, 0, BetaDecoded );
				Frame->Viewport->Canvas->CurX=Frame->Viewport->Canvas->ClipX - 72;
				Frame->Viewport->Canvas->CurY=Frame->Viewport->Canvas->ClipY - 10;
				Frame->Viewport->Canvas->WrappedPrintf( Frame->Viewport->Canvas->SmallFont, 0, BetaDecoded );
			}
		}
/* END BETA VERSION */
#endif

		const INT FramesPerSecond = appRound(CurrentTickRate);
		if( Client->ShowFPS && FramesPerSecond>0 )
		{
			UCanvas* Canvas = Frame->Viewport->Canvas;
			const FLOAT SavedOrgX = Canvas->OrgX;
			const FLOAT SavedOrgY = Canvas->OrgY;
			const FLOAT SavedClipX = Canvas->ClipX;
			const FLOAT SavedClipY = Canvas->ClipY;
			const FLOAT SavedCurX = Canvas->CurX;
			const FLOAT SavedCurY = Canvas->CurY;
			const FColor SavedColor = Canvas->Color;

			Canvas->OrgX = 0.f;
			Canvas->OrgY = 0.f;
			Canvas->ClipX = Canvas->X;
			Canvas->ClipY = Canvas->Y;
			Canvas->CurX = 0.f;
			Canvas->CurY = 0.f;
			INT TextWidth, TextHeight;
			Canvas->WrappedStrLenf( Canvas->SmallFont, TextWidth, TextHeight, TEXT("%d"), FramesPerSecond );
			Canvas->CurX = Canvas->ClipX - TextWidth - 12.f;
			Canvas->CurY = 12.f;
			Canvas->Color = FColor(190,198,204,255);
			Canvas->WrappedPrintf( Canvas->SmallFont, 0, TEXT("%d"), FramesPerSecond );

			Canvas->OrgX = SavedOrgX;
			Canvas->OrgY = SavedOrgY;
			Canvas->ClipX = SavedClipX;
			Canvas->ClipY = SavedClipY;
			Canvas->CurX = SavedCurX;
			Canvas->CurY = SavedCurY;
			Canvas->Color = SavedColor;
		}

		Viewport->Canvas->Render = 0;
		Render->PostRender( Frame );
		Viewport->Unlock( Blit );
		Render->FinishMasterFrame();
	}

	GIsDrawing = 0;
	unguard;
}

void ExportTravel( FOutputDevice& Out, AActor* Actor )
{
	guard(ExportTravel);
	debugf( TEXT("Exporting travelling actor of class %s"), Actor->GetClass()->GetPathName() );//!!xyzzy
	check(Actor);
	if( !Actor->bTravel )
		return;
	Out.Logf( TEXT("Class=%s Name=%s\r\n{\r\n"), Actor->GetClass()->GetPathName(), Actor->GetName() );
	for( TFieldIterator<UProperty> It(Actor->GetClass()); It; ++It )
	{
		for( INT Index=0; Index<It->ArrayDim; Index++ )
		{
			TCHAR Value[1024];
			if
			(	(It->PropertyFlags & CPF_Travel)
			&&	It->ExportText( Index, Value, (BYTE*)Actor, &Actor->GetClass()->Defaults(0), 0 ) )
			{
				Out.Log( It->GetName() );
				if( It->ArrayDim!=1 )
					Out.Logf( TEXT("[%i]"), Index );
				Out.Log( TEXT("=") );
				UObjectProperty* Ref = Cast<UObjectProperty>( *It );
				if( Ref && Ref->PropertyClass->IsChildOf(AActor::StaticClass()) )
				{
					UObject* Obj = *(UObject**)( (BYTE*)Actor + It->Offset + Index*It->ElementSize );
					Out.Logf( TEXT("%s\r\n"), Obj ? Obj->GetName() : TEXT("None") );
				}
				Out.Logf( TEXT("%s\r\n"), Value );
			}
		}
	}
	Out.Logf( TEXT("}\r\n") );
	unguard;
}

//
// Jumping viewport.
//
void UGameEngine::SetClientTravel( UPlayer* Player, const TCHAR* NextURL, UBOOL bItems, ETravelType TravelType )
{
	guard(UGameEngine::SetClientTravel);
	check(Player);

	UViewport* Viewport    = CastChecked<UViewport>( Player );
	Viewport->TravelURL    = NextURL;
	Viewport->TravelType   = TravelType;
	Viewport->bTravelItems = bItems;

	unguard;
}

/*-----------------------------------------------------------------------------
	Tick.
-----------------------------------------------------------------------------*/

//
// Get tick rate limitor.
//
FLOAT UGameEngine::GetMaxTickRate()
{
	guard(UGameEngine::GetMaxTickRate);
	static UBOOL LanPlay = ParseParam(appCmdLine(),TEXT("lanplay"));
	if( GLevel && GLevel->NetDriver && !GIsClient )
		return Clamp( LanPlay ? GLevel->NetDriver->LanServerMaxTickRate : GLevel->NetDriver->NetServerMaxTickRate, 10, 120 );
	else if( GLevel && GLevel->NetDriver && GLevel->NetDriver->ServerConnection )
		return GLevel->NetDriver->ServerConnection->CurrentNetSpeed/64;
	else if( GLevel && GLevel->DemoRecDriver && !GLevel->DemoRecDriver->ServerConnection )
		return Clamp( LanPlay ? GLevel->NetDriver->LanServerMaxTickRate : GLevel->DemoRecDriver->NetServerMaxTickRate, 10, 120 );
	else
		return Max(0.f, FrameRateLimit);
	unguard;
}

//
// Update everything.
//
void UGameEngine::Tick( FLOAT DeltaSeconds )
{
	guard(UGameEngine::Tick);

	// Read or write frame tick for replay.
	if( Replay.Paused() )
		return;
	Replay.SerializeFrameTick( DeltaSeconds );

	INT LocalTickCycles=0;
	clock(LocalTickCycles);

	// If all viewports closed, time to exit.
	if( Client && Client->Viewports.Num()==0 )
	{
		debugf( TEXT("All Windows Closed") );
		appRequestExit( 0 );
		return;
	}

	// If game is paused, release the cursor.
	static UBOOL WasPaused=1;
	if
	(	Client
	&&	Client->Viewports.Num()==1
	&&	GLevel )
	{
		UBOOL IsPaused
		=	GLevel->GetLevelInfo()->Pauser!=TEXT("")
		||	Client->Viewports(0)->Actor->bShowMenu
		||	Client->Viewports(0)->bShowWindowsMouse;
		if( IsPaused && !WasPaused )
			Client->Viewports(0)->SetMouseCapture( 0, 0, 0 );
		else if( WasPaused && !IsPaused && Client->CaptureMouse )
			Client->Viewports(0)->SetMouseCapture( 1, 1, 1 );
		WasPaused = IsPaused;
	}
	else WasPaused=0;

	// Update subsystems.
	UObject::StaticTick();				
	GCache.Tick();

	// Update the level.
	guard(TickLevel);
	GameCycles=0;
	clock(GameCycles);
	if( GLevel )
		GLevel->Tick( LEVELTICK_All, DeltaSeconds );
	if( GEntry && GEntry!=GLevel )
		GEntry->Tick( LEVELTICK_All, DeltaSeconds );
	if( Client && Client->Viewports.Num() && Client->Viewports(0)->Actor->GetLevel()!=GLevel )
		Client->Viewports(0)->Actor->GetLevel()->Tick( LEVELTICK_All, DeltaSeconds );
	unclock(GameCycles);
	unguard;


	// A repair rewrite is deferred until the loaded player has completed
	// possession and travel acceptance in LoadMap.
	guard(SaveRepair);
	if
	(	GRepairSavePending
	&&	GRepairSaveSlot!=INDEX_NONE
	&&	Client
	&&	Client->Viewports.Num()
	&&	Client->Viewports(0)->Actor
	&&	Client->Viewports(0)->Actor->GetLevel()==GLevel )
	{
		const INT RepairSlot = GRepairSaveSlot;
		APlayerPawn* RepairPlayer = Client->Viewports(0)->Actor;
		RepairPlayer->bQueuedToSaveGame = false;
		GRepairSavePending = 0;
		GRepairSaveSlot = INDEX_NONE;
		SaveGame( RepairSlot );
		debugf( NAME_Log, TEXT("Save repair completed: %i"), RepairSlot );
		appRequestExit( 0 );
		return;
	}
	unguard;

	// HP2 specific - Handle player saving
	guard(PlayerSaving);
	APlayerPawn* Player = Client->Viewports(0)->Actor;
	if( Client && Client->Viewports.Num() && Player->bQueuedToSaveGame )
	{
		// We are queued to save the game...
	
		// Make sure we have a path to save to
		GFileManager->MakeDirectory( *GSys->SavePath, 0 );
		GFileManager->MakeDirectory( *GSys->SaveSlotPath, 0 );
		
		//DEBUG
		TCHAR Filename[256];
		appSprintf( Filename, TEXT("%s") PATH_SEPARATOR TEXT("Save%i.usa"), *GSys->SaveSlotPath, 0 );
		debugf( NAME_Log, TEXT("Saving game...(before call) Filename: %s"), Filename );
		
		// Make sure we set this to false BEFORE we save the game.
		Player->bQueuedToSaveGame = false;
		
		// *** SAVE THE GAME
		SaveGame( 0 );
		
		//	The sounds are getting turned off during the save.  THis will restart them.
		//
		Exec(TEXT("RestartSounds"));
		
		// Flush our engine caches ( this fixes some graphical glitches on some vid cards )
		Flush(1);

		return;
	}
	unguard;


	// Handle server travelling.
	guard(ServerTravel);
	if( GLevel && GLevel->GetLevelInfo()->NextURL!=TEXT("") )
	{
		if( (GLevel->GetLevelInfo()->NextSwitchCountdown-=DeltaSeconds) <= 0.f )
		{
			// Travel to new level, and exit.
			TMap<FString,FString> TravelInfo;
			if( GLevel->GetLevelInfo()->NextURL==TEXT("?RESTART") )
			{
				TravelInfo = GLevel->TravelInfo;
			}
			else if( GLevel->GetLevelInfo()->bNextItems )
			{
				TravelInfo = GLevel->TravelInfo;
				for( INT i=0; i<GLevel->Actors.Num(); i++ )
				{
					APlayerPawn* P = Cast<APlayerPawn>( GLevel->Actors(i) );
					if( P && P->Player )
					{
						// Export items and self.
						FStringOutputDevice PlayerTravelInfo;
						ExportTravel( PlayerTravelInfo, P );
						for( AActor* Inv=P->Inventory; Inv; Inv=Inv->Inventory )
							ExportTravel( PlayerTravelInfo, Inv );
						TravelInfo.Set( *P->PlayerReplicationInfo->PlayerName, *PlayerTravelInfo );

						// Prevent local ClientTravel from taking place, since it will happen automatically.
						if( Cast<UViewport>( P->Player ) )
							Cast<UViewport>( P->Player )->TravelURL = TEXT("");
					}
				}
			}
			debugf( TEXT("Server switch level: %s"), *GLevel->GetLevelInfo()->NextURL );
			FString Error;
			Browse( FURL(&LastURL,*GLevel->GetLevelInfo()->NextURL,TRAVEL_Relative), &TravelInfo, Error );
			GLevel->GetLevelInfo()->NextURL = TEXT("");
			return;
		}
	}
	unguard;

	// Handle client travelling.
	guard(ClientTravel);
	if( Client && Client->Viewports.Num() && Client->Viewports(0)->TravelURL!=TEXT("") )
	{
		// Travel to new level, and exit.
		UViewport* Viewport = Client->Viewports( 0 );
		TMap<FString,FString> TravelInfo;

		// Export items.
		if( appStricmp(*Viewport->TravelURL,TEXT("?RESTART"))==0 )
		{
			TravelInfo = GLevel->TravelInfo;
		}
		else if( Viewport->bTravelItems )
		{
			debugf( TEXT("Export travel for: %s"), *Viewport->Actor->PlayerReplicationInfo->PlayerName );
			FStringOutputDevice PlayerTravelInfo;
			ExportTravel( PlayerTravelInfo, Viewport->Actor );
			for( AActor* Inv=Viewport->Actor->Inventory; Inv; Inv=Inv->Inventory )
				ExportTravel( PlayerTravelInfo, Inv );
			TravelInfo.Set( *Viewport->Actor->PlayerReplicationInfo->PlayerName, *PlayerTravelInfo );
		}
		FString Error;
		Browse( FURL(&LastURL,*Viewport->TravelURL,Viewport->TravelType), &TravelInfo, Error );
		Viewport->TravelURL=TEXT("");

		return;
	}
	unguard;

	// Update the pending level.
	guard(TickPending);
	if( GPendingLevel )
	{
		GPendingLevel->Tick( DeltaSeconds );
		if( GPendingLevel->Error!=TEXT("") )
		{
			// Pending connect failed.
			guard(PendingFailed);
			SetProgress( LocalizeError("ConnectionFailed"), *GPendingLevel->Error, 4.f );
			debugf( NAME_Log, LocalizeError("Pending"), *GPendingLevel->URL.String(), *GPendingLevel->Error );
			// Notify console.
			if( GPendingLevel->FailCode!=TEXT("") && Client && Client->Viewports(0) && Client->Viewports(0)->Console )
				Client->Viewports(0)->Console->eventConnectFailure( GPendingLevel->FailCode, GPendingLevel->FailURL );

			delete GPendingLevel;
			GPendingLevel = NULL;
			unguard;
		}
		else if( GPendingLevel->Success && !GPendingLevel->FilesNeeded && !GPendingLevel->SentJoin )
		{
			// Attempt to load the map.
			FString Error;
			guard(AttemptLoadPending);
			LoadMap( GPendingLevel->URL, GPendingLevel, NULL, Error );
			if( Error!=TEXT("") )
			{
				SetProgress( LocalizeError("ConnectionFailed"), *Error, 4.f );
			}
			else if( !GPendingLevel->LonePlayer )
			{
				// Show connecting message, cause precaching to occur.
				GLevel->GetLevelInfo()->LevelAction = LEVACT_Connecting;
				GEntry->GetLevelInfo()->LevelAction = LEVACT_Connecting;
				if( Client )
					Client->Tick();

				// Send join.
				GPendingLevel->SendJoin();
				GPendingLevel->NetDriver = NULL;
				GPendingLevel->DemoRecDriver = NULL;
			}
			unguard;

			// Kill the pending level.
			guard(KillPending);
			delete GPendingLevel;
			GPendingLevel = NULL;
			unguard;
		}
	}
	unguard;

	// Render everything.
	guard(ClientTick);
	INT LocalClientCycles=0;
	if( Client )
	{
		clock(LocalClientCycles);
		UViewport* Viewport=Client->Viewports(0);
		Viewport->Actor->eventViewFlash( DeltaSeconds );
		Client->Tick();
		unclock(LocalClientCycles);
	}
	ClientCycles=LocalClientCycles;
	unguard;

	unclock(LocalTickCycles);
	TickCycles=LocalTickCycles;

	// Replay input here.
	if( Replay.Replaying() )
	{
		FReplay::FInputEvent IE;
		while( Replay.SerializeFrameInput( IE ) )
		{
			if( IE.State == IST_MAX )
			{
				// Special input functions.
				if( IE.iKey == IK_MouseX )
					GViewport->WindowsMouseX = IE.Delta;
				else if( IE.iKey == IK_MouseY )
					GViewport->WindowsMouseY = IE.Delta;
				else
					Key( GViewport, IE.iKey );
			}
			else
				InputEvent( GViewport, IE.iKey, IE.State, IE.Delta );
		}
	}

	GTicks++;
	unguard;
}

/*-----------------------------------------------------------------------------
	Saving the game.
-----------------------------------------------------------------------------*/

//
// Save the current game state to a file.
//
void UGameEngine::SaveGame( INT Position )
{
	guard(UGameEngine::SaveGame);

	TCHAR Filename[256];
	GFileManager->MakeDirectory( *GSys->SavePath, 0 );
	GFileManager->MakeDirectory( *GSys->SaveSlotPath, 0 );
	appSprintf( Filename, TEXT("%s") PATH_SEPARATOR TEXT("Save%i.usa"), *GSys->SaveSlotPath, Position );
	
	debugf( NAME_Log, TEXT("Saving game... filename: %s"), Filename );

	GLevel->GetLevelInfo()->LevelAction=LEVACT_Saving;
	PaintProgress();
	GWarn->BeginSlowTask( LocalizeProgress("Saving"), 1, 0 );
	if( GLevel->BrushTracker )
	{
		delete GLevel->BrushTracker;
		GLevel->BrushTracker = NULL;
	}
	GLevel->CleanupDestroyed( 1 );
	if( SavePackage( GLevel->GetOuter(), GLevel, 0, Filename, GLog ) )
	{
		// Copy the hub stack.
		INT i;
		for( i=0; i<GLevel->GetLevelInfo()->HubStackLevel; i++ )
		{
			TCHAR Src[256], Dest[256];
			appSprintf( Src, TEXT("%s") PATH_SEPARATOR TEXT("Game%i.usa"), *GSys->SaveSlotPath, i );
			appSprintf( Dest, TEXT("%s") PATH_SEPARATOR TEXT("Save%i%i.usa"), *GSys->SaveSlotPath, Position, i );
			GFileManager->Copy( Src, Dest );
		}
		while( 1 )
		{
			appSprintf( Filename, TEXT("%s") PATH_SEPARATOR TEXT("Save%i%i.usa"), *GSys->SaveSlotPath, Position, i++ );
			if( GFileManager->FileSize(Filename)<=0 )
				break;
			GFileManager->Delete( Filename );
		}
	}
	for( INT i=0; i<GLevel->Actors.Num(); i++ )
		if( Cast<AMover>(GLevel->Actors(i)) )
			Cast<AMover>(GLevel->Actors(i))->SavedPos = FVector(-1,-1,-1);
	GLevel->BrushTracker = GNewBrushTracker( GLevel );
	GWarn->EndSlowTask();
	GLevel->GetLevelInfo()->LevelAction=LEVACT_None;
	GCache.Flush();

	// ****
	// Now that we are done saving our game, save all of our pa files from cache into our SaveGamePath
	SavePersistentActorCache();
	// ****

	unguard;
}

/*-----------------------------------------------------------------------------
	Input overrides, incorporating replays and mouse feedback.
-----------------------------------------------------------------------------*/

//
// Mouse delta while dragging.
//
void UGameEngine::MouseDelta( UViewport* Viewport, DWORD ClickFlags, FLOAT DX, FLOAT DY )
{
	// No replay functionality needed.
	guard(UGameEngine::MouseDelta);
	if
	(	(ClickFlags & MOUSE_FirstHit)
	&&	Client
	&&	Client->Viewports.Num()==1
	&&	GLevel
	&&	GLevel->GetLevelInfo()->Pauser==TEXT("")
	&&	!Viewport->Actor->bShowMenu
	&&  !Viewport->bShowWindowsMouse )
	{
		Viewport->SetMouseCapture( 1, 1, 1 );
	}
	else if( (ClickFlags & MOUSE_LastRelease) && !Client->CaptureMouse )
	{
		Viewport->SetMouseCapture( 0, 0, 0 );
	}
	unguard;
}

//
// Absolute mouse position.
//
void UGameEngine::MousePosition( UViewport* Viewport, DWORD ClickFlags, FLOAT X, FLOAT Y )
{
	guard(UGameEngine::MousePosition);

	if( Viewport )
	{
		Viewport->WindowsMouseX = X;
		Viewport->WindowsMouseY = Y;
		if( Replay.Recording() )
		{
			Replay.SerializeFrameInput( IK_MouseX, IST_MAX, X );
			Replay.SerializeFrameInput( IK_MouseY, IST_MAX, Y );
		}
	}

	unguard;
}

//
// Mouse clicking.
//
void UGameEngine::Click( UViewport* Viewport, DWORD ClickFlags, FLOAT X, FLOAT Y )
{
	// No replay functionality needed.
	guard(UGameEngine::Click);
	unguard;
}

UBOOL UGameEngine::Key( UViewport* Viewport, EInputKey Key )
{
	// Intercept, and record handled input keys.
	if( UEngine::Key( Viewport, Key ) )
	{
		if( Replay.Recording() )
			Replay.SerializeFrameInput( Key, IST_MAX );
		return true;
	}
	return false;
}

UBOOL UGameEngine::InputEvent( UViewport* Viewport, EInputKey iKey, EInputAction State, FLOAT Delta )
{
	// Nasty nasty hack for special pause
	if (State == IST_Press && iKey == IK_Delete)
		GLevel->bInSpecialPauseMode = !GLevel->bInSpecialPauseMode;



	if( Replay.Replaying() && !Replay.InReplay() )
	{
		// Ignore real input, except for replay control keys.
		if( State == IST_Press && iKey == IK_Escape )
			Replay.Stop();
		else if( State == IST_Press && iKey == IK_Pause )
			Replay.Pause( !Replay.Paused() );
		else if( State == IST_Press && iKey == IK_ScrollLock )
			if( Replay.Paused() )
				// Single step.
				Replay.Pause( true, 1 );
			
		return true;
	}
	else if( Replay.Recording() )
	{
		if( State == IST_Press && iKey == IK_Backslash )
		{
			Replay.Stop();
			return true;
		}
	}

	// Intercept, and record handled input keys.
	if( Replay.Recording() )
		Replay.SerializeFrameInput( iKey, State, Delta );
	return UEngine::InputEvent( Viewport, iKey, State, Delta );
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
