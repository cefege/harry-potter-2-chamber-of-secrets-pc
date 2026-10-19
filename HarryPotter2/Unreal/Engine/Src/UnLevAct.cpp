/*=============================================================================
	UnLevAct.cpp: Level actor functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"

#include "HP2TraceHooks.h"
#include "hp2_rs.h"

extern void HP2ActorTransitionPreBeginBefore( AActor* Actor );
extern void HP2ActorTransitionPreBeginAfter( AActor* Actor );
extern void HP2ActorTransitionSpawnRequest( UClass* Class, FName Name, AActor* Owner );
extern void HP2ActorTransitionSpawnResult( AActor* Owner, AActor* Child );
extern void HP2ActorTransitionSpawnPublished( AActor* Owner, AActor* Child );

/*-----------------------------------------------------------------------------
	Level actor management.
-----------------------------------------------------------------------------*/

//
// Create a new actor. Returns the new actor, or NULL if failure.
//
AActor* ULevel::SpawnActor
(
	UClass*			Class,
	FName			InName,
	AActor*			Owner,
	class APawn*	Instigator,
	FVector			Location,
	FRotator		Rotation,
	AActor*			Template,
	UBOOL			bNoCollisionFail,
	UBOOL			bRemoteOwned
)
{
	guard(ULevel::SpawnActor);
	HP2ActorTransitionSpawnRequest( Class, InName, Owner );
	HP2CreatureGeneratorTraceSpawnRequest( Class, Owner );


	// Make sure this class is spawnable.
	if( !Class )
	{
		debugf( NAME_Warning, TEXT("SpawnActor failed because no class was specified") );
		HP2ActorTransitionSpawnResult( Owner, NULL );
		HP2CreatureGeneratorTraceSpawnResult( Owner, NULL );

		return NULL;
	}
	if( Class->ClassFlags & CLASS_Abstract )
	{
		debugf( NAME_Warning, TEXT("SpawnActor failed because class %s is abstract"), Class->GetName() );
		HP2ActorTransitionSpawnResult( Owner, NULL );
		HP2CreatureGeneratorTraceSpawnResult( Owner, NULL );

		return NULL;
	}
	else if( !Class->IsChildOf(AActor::StaticClass()) )
	{
		debugf( NAME_Warning, TEXT("SpawnActor failed because %s is not an actor class"), Class->GetName() );
		HP2ActorTransitionSpawnResult( Owner, NULL );
		HP2CreatureGeneratorTraceSpawnResult( Owner, NULL );

		return NULL;
	}
	else if( !GIsEditor && (Class->GetDefaultActor()->bStatic || Class->GetDefaultActor()->bNoDelete) )
	{
		debugf( NAME_Warning, TEXT("SpawnActor failed because class %s has bStatic or bNoDelete"), Class->GetName() );
		HP2ActorTransitionSpawnResult( Owner, NULL );
		HP2CreatureGeneratorTraceSpawnResult( Owner, NULL );

		return NULL;		
	}

	// Use class's default actor as a template.
	if( !Template )
		Template = Class->GetDefaultActor();
	check(Template!=NULL);

	// Make sure actor will fit at desired location, and adjust location if necessary.
	if( (Template->bCollideWorld || (Template->bCollideWhenPlacing && (GetLevelInfo()->NetMode != NM_Client))) && !bNoCollisionFail )
		if( !FindSpot( Template->GetCylinderExtent(), Location, 0, 1 ) )
		{
			debugf( NAME_Warning, TEXT("SpawnActor failed because %s will not fit in location"), Class->GetName() );
			HP2ActorTransitionSpawnResult( Owner, NULL );
			HP2CreatureGeneratorTraceSpawnResult( Owner, NULL );

			return NULL;
		}

	// Add at end of list.
	INT iActor = Actors.Add();
    AActor* Actor = Actors(iActor) = (AActor*)StaticConstructObject( Class, GetOuter(), InName, 0, Template );
	Actor->SetFlags( RF_Transactional );
	HP2ActorTransitionSpawnResult( Owner, Actor );
	HP2CreatureGeneratorTraceSpawnResult( Owner, Actor );


	// Set base actor properties.
	Actor->Tag		= Class->GetFName();
	Actor->Region	= FPointRegion( GetLevelInfo() );
	Actor->Level	= GetLevelInfo();
	Actor->bTicked  = !Ticked;
	Actor->XLevel	= this;

	// Set network role.
	check(Actor->Role==ROLE_Authority);
	if( bRemoteOwned )
		Exchange( Actor->Role, Actor->RemoteRole );

	// Remove the actor's brush, if it has one, because moving brushes are not duplicatable.
	if( Actor->Brush )
		Actor->Brush = NULL;

	// Set the actor's location and rotation.
	Actor->Location = Location;
	Actor->OldLocation = Location;
	Actor->Rotation = Rotation;
	if( Actor->bCollideActors && Hash  )
		Hash->AddActor( Actor );

	// Init the actor's zone.
	Actor->Region = FPointRegion(GetLevelInfo());
	if( Actor->IsA(APawn::StaticClass()) )
		((APawn*)Actor)->FootRegion = ((APawn*)Actor)->HeadRegion = FPointRegion(GetLevelInfo());

	// Set owner.
	Actor->SetOwner( Owner );

	// Set instigator
	Actor->Instigator = Instigator;

	// Send messages.
	Actor->InitExecution();
	Actor->Spawned();
	Actor->eventSpawned();
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorLifecycleEvent( Actor, TEXT("Spawned") );
	HP2ActorTransitionPreBeginBefore( Actor );
	Actor->eventPreBeginPlay();
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorLifecycleEvent( Actor, TEXT("PreBeginPlay") );
	HP2ActorTransitionPreBeginAfter( Actor );
	Actor->eventBeginPlay();
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorLifecycleEvent( Actor, TEXT("BeginPlay") );
	if( Actor->bDeleteMe )
	{
		HP2ActorTransitionSpawnPublished( Owner, NULL );
		HP2CreatureGeneratorTraceSpawnPublished( Owner, NULL );

		return NULL;
	}

	// Set the actor's zone.
	SetActorZone( Actor, iActor==0, 1 );

	// Send PostBeginPlay.
	Actor->eventPostBeginPlay();
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorLifecycleEvent( Actor, TEXT("PostBeginPlay") );

	// Check for encroachment.
	if( !bNoCollisionFail )
	{
		if( Actor->bCollideActors && Hash )
			Hash->RemoveActor( Actor );

		if( CheckEncroachment( Actor, Actor->Location, Actor->Rotation, 0 ) )
		{
			DestroyActor( Actor );
			HP2ActorTransitionSpawnPublished( Owner, NULL );
			HP2CreatureGeneratorTraceSpawnPublished( Owner, NULL );

			return NULL;
		}
		if( Actor->bCollideActors && Hash )
			Hash->AddActor( Actor );
	}

	// Init scripting.
	Actor->eventSetInitialState();
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorLifecycleEvent( Actor, TEXT("SetInitialState") );

	// Find Base
	if( !Actor->Base && Actor->bCollideWorld
		 && (Actor->IsA(ADecoration::StaticClass()) || Actor->IsA(AInventory::StaticClass()) || Actor->IsA(APawn::StaticClass())) 
		 && ((Actor->Physics == PHYS_None) || (Actor->Physics == PHYS_Rotating)) )
		Actor->FindBase();

	// Success: Return the actor.
	if( InTick )
		NewlySpawned = new(GEngineMem)FActorLink(Actor,NewlySpawned);

	static UBOOL InsideNotification = 0;
	if( !InsideNotification )
	{
		InsideNotification = 1;
		// Spawn notification
		for( ASpawnNotify* N = GetLevelInfo()->SpawnNotify; N; N = N->Next )
		{
			if( N->ActorClass && Actor->IsA(N->ActorClass) )
				Actor = N->eventSpawnNotification( Actor );
		}
		InsideNotification = 0;
	}
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorSpawned( Actor );
	HP2ActorTransitionSpawnPublished( Owner, Actor );
	HP2CreatureGeneratorTraceSpawnPublished( Owner, Actor );

	return Actor;
	unguardf(( TEXT("(%s)"), Class->GetName() ));
}

//
// Spawn a brush.
//
ABrush* ULevel::SpawnBrush()
{
	guard(ULevel::SpawnBrush);

	ABrush* Result = (ABrush*)SpawnActor( ABrush::StaticClass() );
	check(Result);

	return Result;
	unguard;
}

//
// Destroy an actor.
// Returns 1 if destroyed, 0 if it couldn't be destroyed.
//
// What this routine does:
// * Remove the actor from the actor list.
// * Generally cleans up the engine's internal state.
//
// What this routine does not do, but is done in ULevel::Tick instead:
// * Removing references to this actor from all other actors.
// * Killing the actor resource.
//
// This routine is set up so that no problems occur even if the actor
// being destroyed inside its recursion stack.
//
UBOOL ULevel::DestroyActor( AActor* ThisActor, UBOOL bNetForce )
{
	guard(ULevel::DestroyActor);
	check(ThisActor);
	check(ThisActor->IsValid());
	//debugf( NAME_Log, "Destroy %s", ThisActor->GetClass()->GetName() );

	// In-game deletion rules.
	guard(DestroyRules);
	if( !GIsEditor )
	{
		// Can't kill bStatic and bNoDelete actors during play.
		if( ThisActor->bStatic || ThisActor->bNoDelete )
			return 0;

		// If already on list to be deleted, pretend the call was successful.
		if( ThisActor->bDeleteMe )
			return 1;

		// Can't kill if wrong role.
		if( ThisActor->Role!=ROLE_Authority && !bNetForce && !ThisActor->bNetTemporary )
			return 0;

		// Don't destroy player actors.
		APlayerPawn* P = Cast<APlayerPawn>( ThisActor );
		if( P )
		{
			UNetConnection* C = Cast<UNetConnection>(P->Player);
			if( C && C->Channels[0] && C->State!=USOCK_Closed )
			{
				C->Channels[0]->Close();
				return 0;
			}
		}
	}
	unguard;

	// Get index.
	INT iActor = GetActorIndex( ThisActor );
	guard(ModifyActor);
	Actors.ModifyItem( iActor );
	ThisActor->Modify();
	unguard;

	// Send EndState notification.
	guard(EndState);
	if( ThisActor->GetStateFrame() && ThisActor->GetStateFrame()->StateNode && ThisActor->IsProbing(NAME_EndState) )
	{
		ThisActor->eventEndState();
		if( ThisActor->bDeleteMe )
			return 1;
	}
	unguard;

	// Remove from base.
	guard(Debase);
	if( ThisActor->Base )
	{
		ThisActor->SetBase( NULL );
		if( ThisActor->bDeleteMe )
			return 1;
	}
	if( ThisActor->StandingCount > 0 )
		for( INT i=0; i<Actors.Num(); i++ )
			if( Actors(i) && Actors(i)->Base == ThisActor ) 
				Actors(i)->SetBase( NULL );
	unguard;

	// Remove from world collision hash.
	guard(Unhash);
	if( Hash )
	{
		if( ThisActor->bCollideActors )
			Hash->RemoveActor( ThisActor, true );
		Hash->CheckActorNotReferenced( ThisActor );
	}
	unguard;

	// Tell this actor it's about to be destroyed.
	guard(ProcessDestroyed);
	ThisActor->eventDestroyed();
	if( ThisActor->bDeleteMe )
		return 1;
	unguard;

	// Clean up all owned and touching actors.
	guard(CleanupStandardRefs);
	for( INT iActor=0; iActor<Actors.Num(); iActor++ )
	{
		AActor* Other = Actors(iActor);
		if( Other )
		{
			if( Other->TickParent==ThisActor )
				Other->TickParent = NULL;

			if( Other->TickParent2==ThisActor )
				Other->TickParent2 = NULL;

			if( Other->Owner==ThisActor )
			{
				Other->SetOwner( NULL );
				if( ThisActor->bDeleteMe )
					return 1;
			}
			else
			{
				for( INT j=0; j<ARRAY_COUNT(Other->Touching); j++ )
				{
					if( Other->Touching[j]==ThisActor )
					{
						ThisActor->EndTouch( Other, 1 );
						if( ThisActor->bDeleteMe )
							return 1;
					}
				}
			}
		}
	}
	unguard;

	// If this actor has an owner, notify it that it has lost a child.
	guard(Disown);
	if( ThisActor->Owner )
	{
		ThisActor->Owner->eventLostChild( ThisActor );
		if( ThisActor->bDeleteMe )
			return 1;
	}
	unguard;

	// Notify net players that this guy has been destroyed.
	if( NetDriver )
		NetDriver->NotifyActorDestroyed( ThisActor );

	// If demo recording, notify the demo.
	if( DemoRecDriver && !DemoRecDriver->ServerConnection )
		DemoRecDriver->NotifyActorDestroyed( ThisActor );

	// Remove the actor from the actor list.
	guard(Unlist);
	check(Actors(iActor)==ThisActor);
	Actors(iActor) = NULL;
	ThisActor->bDeleteMe = 1;
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorDestroying( ThisActor );
	unguard;

	// Do object destroy.
	guard(ShutupSound);
	if( Engine && Engine->Audio )
		Engine->Audio->NoteDestroy( ThisActor );
	ThisActor->ConditionalDestroy();
	unguard;

	// Cleanup.
	guard(Cleanup);
	if( !GIsEditor )
	{
		// During play, just add to delete-list list and destroy when level is unlocked.
		ThisActor->Deleted = FirstDeleted;
		FirstDeleted       = ThisActor;
	}
	else
	{
		// Destroy them now.
		CleanupDestroyed( 1 );
	}
	unguard;

	// Return success.
	return 1;
	unguardf(( TEXT("(%s)"), ThisActor->GetFullName() ));
}

//
// Compact the actor list.
//
void ULevel::CompactActors()
{
	guard(ULevel::CompactActors);
	INT c = iFirstDynamicActor;
	for( INT i=iFirstDynamicActor; i<Actors.Num(); i++ )
	{
		if( Actors(i) )
		{
			if( !Actors(i)->bDeleteMe )
				Actors(c++) = Actors(i);
			else
				debugf( TEXT("Undeleted %s"), Actors(i)->GetFullName() );
		}
	}
	if( c != Actors.Num() )
		Actors.Remove( c, Actors.Num()-c );
	unguard;
}

//
// Cleanup destroyed actors.
// During gameplay, called in ULevel::Unlock.
// During editing, called after each actor is deleted.
//
void ULevel::CleanupDestroyed( UBOOL bForce )
{
	guard(ULevel::CleanupDestroyed);

	// Pack actor list.
	if( !GIsEditor && !bForce )
		CompactActors();

	// If nothing deleted, exit.
	if( !FirstDeleted )
		return;

	// Don't do anything unless a bunch of actors are in line to be destroyed.
	guard(CheckDeleted);
	INT c=0;
	for( AActor* A=FirstDeleted; A; A=A->Deleted )
		c++;
	if( c<128 && !bForce )
		return;
	unguard;

	// Remove all references to actors tagged for deletion.
	guard(CleanupRefs);
	for( INT iActor=0; iActor<Actors.Num(); iActor++ )
	{
		AActor* Actor = Actors(iActor);
		if( Actor )
		{
			// Would be nice to say if(!Actor->bStatic), but we can't count on it.
			checkSlow(!Actor->bDeleteMe);
			Actor->GetClass()->CleanupDestroyed( (BYTE*)Actor );
		}
	}
	unguard;

	// If editor, let garbage collector destroy objects.
	if( GIsEditor )
		return;

	guard(FinishDestroyedActors);
	while( FirstDeleted!=NULL )
	{
		// Physically destroy the actor-to-delete.
		check(FirstDeleted->bDeleteMe);
		AActor* ActorToKill = FirstDeleted;
		FirstDeleted        = FirstDeleted->Deleted;
		check(ActorToKill->bDeleteMe);

		// Destroy the actor.
		delete ActorToKill;
	}
	unguard;

	unguard;
}

/*-----------------------------------------------------------------------------
	Player spawning.
-----------------------------------------------------------------------------*/

//
// Find an available camera actor in the level and return it, or spawn a new
// one if none are available.  Returns actor number or NULL if none are
// available.
//
void ULevel::SpawnViewActor( UViewport* Viewport )
{
	guard(ULevel::SpawnViewActor);
	check(Engine->Client);
	check(Viewport->Actor==NULL);

	// Find an existing camera actor.
	guard(FindExisting);
	for( INT iActor=0; iActor<Actors.Num(); iActor++ )
	{
		ACamera* TestActor = Cast<ACamera>( Actors(iActor) );
		if( TestActor && !TestActor->Player && (Viewport->GetFName()==TestActor->Tag) ) 
		{
			Viewport->Actor = TestActor;
            break;
		}
    }
	unguard;

	guard(SpawnNew);
    if( !Viewport->Actor )
	{
		// None found, spawn a new one and set default position.
		Viewport->Actor = (ACamera*)SpawnActor( ACamera::StaticClass(), NAME_None, NULL, NULL, FVector(-500,-300,+300), FRotator(0,0,0), NULL, 1 );
		check(Viewport->Actor);
		Viewport->Actor->Tag = Viewport->GetFName();
	}
	unguard;

	// Set the new actor's properties.
	guard(SetProperties);
	Viewport->Actor->SetFlags( RF_NotForClient | RF_NotForServer );
	Viewport->Actor->ClearFlags( RF_Transactional );
	Viewport->Actor->Player		= Viewport;
	Viewport->Actor->ShowFlags	= SHOW_Frame | SHOW_MovingBrushes | SHOW_Actors | SHOW_Brush;
	Viewport->Actor->RendMap    = REN_DynLight;
	Viewport->Actor->bAdmin     = 1;
	unguard;

	// Set the zone.
	SetActorZone( Viewport->Actor, 0, 1 );

	unguard;
}

//
// Spawn an actor for gameplay.
//
struct FAcceptInfo
{
	AActor*			Actor;
	FString			Name;
	TArray<FString> Parms;
	FAcceptInfo( AActor* InActor, const TCHAR* InName )
	: Actor( InActor ), Name( InName ), Parms()
	{}
};
APlayerPawn* ULevel::SpawnPlayActor( UPlayer* Player, ENetRole RemoteRole, const FURL& URL, FString& Error )
{
	guard(ULevel::SpawnPlayActor);
	Error=TEXT("");

	// Get package map.
	UPackageMap*    PackageMap = NULL;
	UNetConnection* Conn       = Cast<UNetConnection>( Player );
	if( Conn )
		PackageMap = Conn->PackageMap;

	// Get PlayerClass.
	UClass* PlayerClass=NULL;
	const TCHAR* Str = URL.GetOption( TEXT("CLASS="), NULL );
	if( Str )
		PlayerClass = StaticLoadClass( APlayerPawn::StaticClass(), NULL, Str, NULL, LOAD_NoWarn, PackageMap );
	if( !PlayerClass )
		PlayerClass = StaticLoadClass( APlayerPawn::StaticClass(), NULL, TEXT("usr:DefaultPlayer.Class"), NULL, LOAD_NoWarn, PackageMap );
	if( !PlayerClass )
		PlayerClass = StaticLoadClass( APlayerPawn::StaticClass(), NULL, TEXT("ini:URL.Class"), NULL, LOAD_NoWarn, PackageMap );
	if( !PlayerClass )
		appErrorf( TEXT("%s"), LocalizeError("LoadPlayerClass") );

	// Make the option string.
	TCHAR Options[1024]=TEXT("");
	for( INT i=0; i<URL.Op.Num(); i++ )
	{
		appStrcat( Options, TEXT("?") );
		appStrcat( Options, *URL.Op(i) );
	}

	// Tell UnrealScript to log in.
//	INT SavedActorCount = Actors.Num();//oldver: Login should say whether to accept inventory.
	APlayerPawn* Actor = GetLevelInfo()->Game->eventLogin( *URL.Portal, Options, Error, PlayerClass );
	if( !Actor )
	{
		debugf( NAME_Warning, TEXT("Login failed: %s"), *Error);
		return NULL;
	}
	// Save packages restore serialized inventory; stale travel state must not be replayed or resaved.
	const UBOOL LoadingGame = URL.HasOption(TEXT("load")) || URL.GetOption(TEXT("load="),NULL) != NULL;
	UBOOL AcceptInventory = !LoadingGame;
	if( LoadingGame )
		TravelInfo.Empty();


	// Possess the newly-spawned player.
	Actor->SetPlayer( Player );
	Actor->Role       = ROLE_Authority;
	Actor->RemoteRole = RemoteRole;
	Actor->ShowFlags  = SHOW_Backdrop | SHOW_Actors | SHOW_PlayerCtrl | SHOW_RealTime;
	Actor->RendMap	  = REN_DynLight;
	if( ParseParam(appCmdLine(),TEXT("alladmin")) || !NetDriver )
		Actor->bAdmin = 1;
	Actor->eventTravelPreAccept();

	// Any saved items?
	Str = NULL;
	if( AcceptInventory )
	{
		const TCHAR* PlayerName = URL.GetOption( TEXT("NAME="), *FURL::DefaultName );
		if( PlayerName )
		{
			FString* FoundItems = TravelInfo.Find( PlayerName );
			if( FoundItems )
				Str = **FoundItems;
		}
		if( !Str && GetLevelInfo()->NetMode==NM_Standalone )
		{
			TMap<FString,FString>::TIterator It(TravelInfo);
			if( It )
				Str = *It.Value();
		}
	}

	// Handle inventory items.
	TCHAR ClassName[256], ActorName[256];
	TArray<FAcceptInfo> Accepted;
	while( Str && Parse(Str,TEXT("CLASS="),ClassName,ARRAY_COUNT(ClassName)) && Parse(Str,TEXT("NAME="),ActorName,ARRAY_COUNT(ActorName)) )
	{
		// Load class.
		debugf( TEXT("Incoming travelling actor of class %s"), ClassName );//!!xyzzy
		FAcceptInfo* Accept=NULL;
		AActor* Spawned=NULL;
		UClass* Class=StaticLoadClass( AActor::StaticClass(), NULL, ClassName, NULL, LOAD_NoWarn|LOAD_AllowDll, PackageMap );
		if( !Class )
		{
			debugf( NAME_Log, TEXT("SpawnPlayActor: Cannot accept travelling class '%s'"), ClassName );
		}
		else if( Class->IsChildOf(APlayerPawn::StaticClass()) )
		{
			Accept = new(Accepted)FAcceptInfo(Actor,ActorName);
		}
		else if( (Spawned=SpawnActor( Class, NAME_None, Actor, NULL, Actor->Location, Actor->Rotation, NULL, 1 ))==NULL )
		{
			debugf( NAME_Log, TEXT("SpawnPlayActor: Failed to spawn travelling class '%s'"), ClassName );
		}
		else
		{
			debugf( NAME_Log, TEXT("SpawnPlayActor: Spawned travelling actor") );
			Accept = new(Accepted)FAcceptInfo(Spawned,ActorName);
		}

		// Save properties.
		TCHAR Buffer[256];
		ParseLine(&Str,Buffer,ARRAY_COUNT(Buffer),1);
		ParseLine(&Str,Buffer,ARRAY_COUNT(Buffer),1);
		while( ParseLine(&Str,Buffer,ARRAY_COUNT(Buffer),1) && appStrcmp(Buffer,TEXT("}"))!=0 )
			if( Accept )
				new(Accept->Parms)FString(Buffer);
	}

	// Import properties.
	for( INT i=0; i<Accepted.Num(); i++ )
	{
		// Parse all properties.
		for( INT j=0; j<Accepted(i).Parms.Num(); j++ )
		{
			const TCHAR* Ptr = *Accepted(i).Parms(j);
			while( *Ptr==' ' )
				Ptr++;
			TCHAR VarName[256], *VarEnd=VarName;
			while( appIsAlnum(*Ptr) || *Ptr=='_' )
				*VarEnd++ = *Ptr++;
			*VarEnd=0;
			INT Element=0;
			if( *Ptr=='[' )
			{
				Element=appAtoi(++Ptr);
				while( appIsDigit(*Ptr) )
					Ptr++;
				if( *Ptr++!=']' )
					continue;
			}
			if( *Ptr++!='=' )
				continue;
			for( TFieldIterator<UProperty> It(Accepted(i).Actor->GetClass()); It; ++It )
			{
				if
				(	(It->PropertyFlags & CPF_Travel)
				&&	appStricmp( It->GetName(), VarName )==0 
				&&	Element<It->ArrayDim )
				{
					// Import the property.
					BYTE* Data = (BYTE*)Accepted(i).Actor + It->Offset + Element*It->ElementSize;
					UObjectProperty* Ref = Cast<UObjectProperty>( *It );
					if( Ref && Ref->PropertyClass->IsChildOf(AActor::StaticClass()) )
					{
						for( INT k=0; k<Accepted.Num(); k++ )
						{
							if( Accepted(k).Name==Ptr )
							{
								*(UObject**)Data = Accepted(k).Actor;
								break;
							}
						}
					}
					else It->ImportText( Ptr, Data, 0 );
				}
			}
		}
	}

	// Call travel-acceptance functions in reverse order to avoid inventory flipping.
	for( INT i=Accepted.Num()-1; i>=0; i-- )
		Accepted(i).Actor->eventTravelPreAccept();
	GetLevelInfo()->Game->eventAcceptInventory( Actor );
	for( INT i=Accepted.Num()-1; i>=0; i-- )
		Accepted(i).Actor->eventTravelPostAccept();
	UStrProperty* PreviousLevelName = FindField<UStrProperty>( Actor->GetClass(), TEXT("PreviousLevelName") );
	if( LoadingGame && PreviousLevelName && (PreviousLevelName->PropertyFlags & CPF_Travel) )
		*(FString*)((BYTE*)Actor + PreviousLevelName->Offset) = TEXT("");
	Actor->eventTravelPostAccept();
	GetLevelInfo()->Game->eventPostLogin( Actor );

	return Actor;
	unguard;
}

/*-----------------------------------------------------------------------------
	Level actor moving/placing.
-----------------------------------------------------------------------------*/

//
// Find a suitable nearby location to place a collision box.
// No suitable location will ever be found if Location is not a valid point inside the level ( and not in 
// a wall)

/*-----------------------------------------------------------------------------
	Rust check services (`LevActServices` in crates/hp2-engine/src/levact.rs,
	same field order). The thunks perform the C++ owner calls.
-----------------------------------------------------------------------------*/

struct Hp2LevActServices
{
	FCheckResult* (*AllocCheckResults)( void* Mem, INT Count );
	INT (*ModelPointCheck)( void* Level, FCheckResult* Hit, const FVector* Location, const FVector* Extent );
	INT (*ModelLineCheck)( void* Level, FCheckResult* Hit, const FVector* End, const FVector* Start, const FVector* Extent, DWORD Flags );
	FCheckResult* (*HashActorPointCheck)( void* Hash, void* Mem, const FVector* Location, const FVector* Extent, DWORD Flags );
	FCheckResult* (*HashActorLineCheck)( void* Hash, void* Mem, const FVector* End, const FVector* Start, const FVector* Extent, DWORD Flags );
	INT (*ActorIsA)( const void* Actor, INT Kind );
	INT (*ActorIsOwnedBy)( const void* Actor, const void* Other );
	INT (*ActorIsBlockedBy)( const void* Actor, const void* Other );
	void (*MemMarkSave)( void* Mem, void* Buffer );
	void (*MemMarkPop)( void* Buffer );
	DWORD (*AppCycles)();
	INT (*IsEditor)();
	void (*BumpObserver)( void* Recipient, void* Other );
	FLOAT (*SinTab)( INT Angle );
	FLOAT (*CosTab)( INT Angle );
	void (*ModelPointRegion)( void* Model, void* Zone, const FVector* Location, void* Out );
	void (*HashAddActor)( void* Hash, void* Actor );
	void (*HashRemoveActor)( void* Hash, void* Actor );
	FCheckResult* (*HashActorEncroachmentCheck)( void* Hash, void* Mem, void* Actor, const FVector* Location, const FRotator* Rotation, DWORD Flags );
	void (*TrackerUpdate)( void* Tracker, void* Actor );
	void (*ActorBeginTouch)( void* Actor, void* Other );
	void (*ActorEndTouch)( void* Actor, void* Other, INT NoNotify );
	INT (*ActorIsOverlapping)( const void* Actor, const void* Other );
	void (*ActorCollisionBox)( const void* Actor, FVector* Min, FVector* Max );
	void (*ActorCylinderExtent)( const void* Actor, FVector* Out );
	void (*ActorSetBase)( void* Actor, void* Base );
	INT (*ActorMoveSmooth)( void* Actor, const FVector* Delta );
	void (*EventBump)( void* Recipient, void* Other );
	INT (*EventEncroachingOn)( void* Actor, void* Other );
	void (*EventEncroachedBy)( void* Actor, void* Other );
	void (*EventZoneChange)( void* Actor, void* Zone );
	void (*ZoneEventActorLeaving)( void* Zone, void* Actor );
	void (*ZoneEventActorEntered)( void* Zone, void* Actor );
	void (*PawnEventFootZoneChange)( void* Pawn, void* Zone );
	void (*PawnEventHeadZoneChange)( void* Pawn, void* Zone );
	void (*PriSetPlayerZone)( void* Pri, void* Zone );
	BYTE (*LevelNetMode)( const void* LevelInfo );
};

static FCheckResult* Hp2LevActAllocCheckResults( void* Mem, INT Count )
{
	return new(*(FMemStack*)Mem, Count) FCheckResult;
}

static INT Hp2LevActModelPointCheck( void* Level, FCheckResult* Hit, const FVector* Location, const FVector* Extent )
{
	return ((ALevelInfo*)Level)->GetLevel()->Model->PointCheck( *Hit, NULL, *Location, *Extent, 0 );
}

static INT Hp2LevActModelLineCheck( void* Level, FCheckResult* Hit, const FVector* End, const FVector* Start, const FVector* Extent, DWORD Flags )
{
	return ((ALevelInfo*)Level)->GetLevel()->Model->LineCheck( *Hit, NULL, *End, *Start, *Extent, (BYTE)Flags );
}

static FCheckResult* Hp2LevActHashActorPointCheck( void* Hash, void* Mem, const FVector* Location, const FVector* Extent, DWORD Flags )
{
	return ((FCollisionHashBase*)Hash)->ActorPointCheck( *(FMemStack*)Mem, *Location, *Extent, Flags );
}

static FCheckResult* Hp2LevActHashActorLineCheck( void* Hash, void* Mem, const FVector* End, const FVector* Start, const FVector* Extent, DWORD Flags )
{
	return ((FCollisionHashBase*)Hash)->ActorLineCheck( *(FMemStack*)Mem, *End, *Start, *Extent, (BYTE)Flags );
}

// Kind: 0 = ALevelInfo, 1 = AMover, 2 = AZoneInfo, 3 = ABrush, 4 = APawn, 5 = ADecoration (ACTOR_KIND_* in levact.rs).
static INT Hp2LevActActorIsA( const void* Actor, INT Kind )
{
	AActor* A = (AActor*)Actor;
	switch( Kind )
	{
		case 0: return A->IsA(ALevelInfo::StaticClass());
		case 1: return A->IsA(AMover::StaticClass());
		case 2: return A->IsA(AZoneInfo::StaticClass());
		case 3: return A->IsA(ABrush::StaticClass());
		case 4: return A->IsA(APawn::StaticClass());
		case 5: return A->IsA(ADecoration::StaticClass());
	}
	return 0;
}

static INT Hp2LevActActorIsOwnedBy( const void* Actor, const void* Other )
{
	return ((AActor*)Actor)->IsOwnedBy( (AActor*)Other );
}

static INT Hp2LevActActorIsBlockedBy( const void* Actor, const void* Other )
{
	return ((AActor*)Actor)->IsBlockedBy( (AActor*)Other );
}

static void Hp2LevActMemMarkSave( void* Mem, void* Buffer )
{
	*(FMemMark*)Buffer = FMemMark( *(FMemStack*)Mem );
}

static void Hp2LevActMemMarkPop( void* Buffer )
{
	((FMemMark*)Buffer)->Pop();
}

static DWORD Hp2LevActAppCycles()
{
	return appCycles();
}

static INT Hp2LevActIsEditor()
{
	return GIsEditor;
}

static void Hp2LevActBumpObserver( void* Recipient, void* Other )
{
	if( GActorMovementObserver )
		GActorMovementObserver->OnActorBump( (AActor*)Recipient, (AActor*)Other );
}

static FLOAT Hp2LevActSinTab( INT Angle )
{
	return GMath.SinTab( Angle );
}

static FLOAT Hp2LevActCosTab( INT Angle )
{
	return GMath.CosTab( Angle );
}

static void Hp2LevActModelPointRegion( void* Model, void* Zone, const FVector* Location, void* Out )
{
	*(FPointRegion*)Out = ((UModel*)Model)->PointRegion( (AZoneInfo*)Zone, *Location );
}

static void Hp2LevActHashAddActor( void* Hash, void* Actor )
{
	((FCollisionHashBase*)Hash)->AddActor( (AActor*)Actor );
}

static void Hp2LevActHashRemoveActor( void* Hash, void* Actor )
{
	((FCollisionHashBase*)Hash)->RemoveActor( (AActor*)Actor );
}

static FCheckResult* Hp2LevActHashActorEncroachmentCheck( void* Hash, void* Mem, void* Actor, const FVector* Location, const FRotator* Rotation, DWORD Flags )
{
	return ((FCollisionHashBase*)Hash)->ActorEncroachmentCheck( *(FMemStack*)Mem, (AActor*)Actor, *Location, *Rotation, Flags );
}

static void Hp2LevActTrackerUpdate( void* Tracker, void* Actor )
{
	UpdateMovingBrushTracker( (FMovingBrushTrackerBase*)Tracker, (AActor*)Actor );
}

static void Hp2LevActActorBeginTouch( void* Actor, void* Other )
{
	((AActor*)Actor)->BeginTouch( (AActor*)Other );
}

static void Hp2LevActActorEndTouch( void* Actor, void* Other, INT NoNotify )
{
	((AActor*)Actor)->EndTouch( (AActor*)Other, NoNotify );
}

static INT Hp2LevActActorIsOverlapping( const void* Actor, const void* Other )
{
	return ((AActor*)Actor)->IsOverlapping( (AActor*)Other );
}

static void Hp2LevActActorCollisionBox( const void* Actor, FVector* Min, FVector* Max )
{
	FBox Box = ((AActor*)Actor)->GetPrimitive()->GetCollisionBoundingBox( (AActor*)Actor, true );
	*Min = Box.Min;
	*Max = Box.Max;
}

static void Hp2LevActActorCylinderExtent( const void* Actor, FVector* Out )
{
	*Out = ((AActor*)Actor)->GetCylinderExtent();
}

static void Hp2LevActActorSetBase( void* Actor, void* Base )
{
	((AActor*)Actor)->SetBase( (AActor*)Base );
}

static INT Hp2LevActActorMoveSmooth( void* Actor, const FVector* Delta )
{
	return ((AActor*)Actor)->moveSmooth( *Delta );
}

static void Hp2LevActEventBump( void* Recipient, void* Other )
{
	((AActor*)Recipient)->eventBump( (AActor*)Other );
}

static INT Hp2LevActEventEncroachingOn( void* Actor, void* Other )
{
	return ((AActor*)Actor)->eventEncroachingOn( (AActor*)Other );
}

static void Hp2LevActEventEncroachedBy( void* Actor, void* Other )
{
	((AActor*)Actor)->eventEncroachedBy( (AActor*)Other );
}

static void Hp2LevActEventZoneChange( void* Actor, void* Zone )
{
	((AActor*)Actor)->eventZoneChange( (AZoneInfo*)Zone );
}

static void Hp2LevActZoneEventActorLeaving( void* Zone, void* Actor )
{
	((AZoneInfo*)Zone)->eventActorLeaving( (AActor*)Actor );
}

static void Hp2LevActZoneEventActorEntered( void* Zone, void* Actor )
{
	((AZoneInfo*)Zone)->eventActorEntered( (AActor*)Actor );
}

static void Hp2LevActPawnEventFootZoneChange( void* Pawn, void* Zone )
{
	((APawn*)Pawn)->eventFootZoneChange( (AZoneInfo*)Zone );
}

static void Hp2LevActPawnEventHeadZoneChange( void* Pawn, void* Zone )
{
	((APawn*)Pawn)->eventHeadZoneChange( (AZoneInfo*)Zone );
}

static void Hp2LevActPriSetPlayerZone( void* Pri, void* Zone )
{
	((APlayerReplicationInfo*)Pri)->PlayerZone = (AZoneInfo*)Zone;
}

static BYTE Hp2LevActLevelNetMode( const void* LevelInfo )
{
	return ((ALevelInfo*)LevelInfo)->NetMode;
}

static const Hp2LevActServices GHp2LevActServices =
{
	Hp2LevActAllocCheckResults,
	Hp2LevActModelPointCheck,
	Hp2LevActModelLineCheck,
	Hp2LevActHashActorPointCheck,
	Hp2LevActHashActorLineCheck,
	Hp2LevActActorIsA,
	Hp2LevActActorIsOwnedBy,
	Hp2LevActActorIsBlockedBy,
	Hp2LevActMemMarkSave,
	Hp2LevActMemMarkPop,
	Hp2LevActAppCycles,
	Hp2LevActIsEditor,
	Hp2LevActBumpObserver,
	Hp2LevActSinTab,
	Hp2LevActCosTab,
	Hp2LevActModelPointRegion,
	Hp2LevActHashAddActor,
	Hp2LevActHashRemoveActor,
	Hp2LevActHashActorEncroachmentCheck,
	Hp2LevActTrackerUpdate,
	Hp2LevActActorBeginTouch,
	Hp2LevActActorEndTouch,
	Hp2LevActActorIsOverlapping,
	Hp2LevActActorCollisionBox,
	Hp2LevActActorCylinderExtent,
	Hp2LevActActorSetBase,
	Hp2LevActActorMoveSmooth,
	Hp2LevActEventBump,
	Hp2LevActEventEncroachingOn,
	Hp2LevActEventEncroachedBy,
	Hp2LevActEventZoneChange,
	Hp2LevActZoneEventActorLeaving,
	Hp2LevActZoneEventActorEntered,
	Hp2LevActPawnEventFootZoneChange,
	Hp2LevActPawnEventHeadZoneChange,
	Hp2LevActPriSetPlayerZone,
	Hp2LevActLevelNetMode,
};

// AdjustSpot used by FindSpot
void ULevel::AdjustSpot( FVector& Adjusted, FVector TraceDest, FLOAT TraceLen, FCheckResult& Hit )
{
	hp2_rs_engine_levact_adjust_spot( &GHp2LevActServices, &GMem, Hash, this, &Adjusted, &TraceDest, TraceLen, &Hit );
}

UBOOL ULevel::FindSpot
(
	FVector  Extent,
	FVector& Location,
	UBOOL	 bCheckActors,
	UBOOL	 bAssumeFit
)
{
	guard(ULevel::FindSpot);
	return hp2_rs_engine_levact_find_spot( &GHp2LevActServices, &GMem, Hash, this, &Extent, &Location, bCheckActors, bAssumeFit );
	unguard;
}

//
// Try to place an actor that has moved a long way.  This is for
// moving actors through teleporters, adding them to levels, and
// starting them out in levels.  The results of this function is independent
// of the actor's current location and rotation.
//
// If the actor doesn't fit exactly in the location specified, tries
// to slightly move it out of walls and such.
//
// Returns 1 if the actor has been successfully moved, or 0 if it couldn't fit.
//
// Updates the actor's Zone and sends ZoneChange if it changes.
//
UBOOL ULevel::FarMoveActor( AActor* Actor, FVector DestLocation,  UBOOL test, UBOOL bNoCheck )
{
	guard(ULevel::FarMoveActor);
	check(Actor!=NULL);
	return hp2_rs_engine_levact_far_move_actor( &GHp2LevActServices, &GMem, Hash, this, Actor, &DestLocation, test, bNoCheck );
	unguard;
}

//
// Place the actor on the floor below.  May move the actor a long way down.
// Updates the actor's Zone and sends ZoneChange if it changes.
//
//

UBOOL ULevel::DropToFloor( AActor *Actor)
{
	guard(ULevel::DropToFloor);
	check(Actor!=NULL);

	// Try moving down a long way and see if we hit the floor.

	FCheckResult Hit(1.f);
	MoveActor( Actor, FVector( 0, 0, -1000 ), Actor->Rotation, Hit );
	return (Hit.Time < 1.f);

	unguard;
}

//
// Tries to move the actor by a movement vector.  If no collision occurs, this function 
// just does a Location+=Move.
//
// Assumes that the actor's Location is valid and that the actor
// does fit in its current Location. Assumes that the level's 
// Dynamics member is locked, which will always be the case during
// a call to ULevel::Tick; if not locked, no actor-actor collision
// checking is performed.
//
// If bCollideWorld, checks collision with the world.
//
// For every actor-actor collision pair:
//
// If both have bCollideActors and bBlocksActors, performs collision
//    rebound, and dispatches Touch messages to touched-and-rebounded 
//    actors.  
//
// If both have bCollideActors but either one doesn't have bBlocksActors,
//    checks collision with other actors (but lets this actor 
//    interpenetrate), and dispatches Touch and UnTouch messages.
//
// Returns 1 if some movement occured, 0 if no movement occured.
//
// Updates actor's Zone and sends ZoneChange if it changes.
//
// If Test = 1 (default 0), do not send notifications.
//
UBOOL ULevel::MoveActor
(
	AActor*			Actor,
	FVector			Delta,
	FRotator		NewRotation,
	FCheckResult&	Hit,
	UBOOL			bTest,
	UBOOL			bIgnorePawns,
	UBOOL			bIgnoreBases,
	UBOOL			bNoFail
)
{
	guard(ULevel::MoveActor);
	check(Actor!=NULL);
	return hp2_rs_engine_levact_move_actor( &GHp2LevActServices, &GMem, Hash, this, Actor, &Delta, &NewRotation, &Hit, bTest, bIgnorePawns, bIgnoreBases, bNoFail );
	unguard;
}

/*-----------------------------------------------------------------------------
	Encroachment.
-----------------------------------------------------------------------------*/

//
// Check whether Actor is encroaching other actors after a move, and return
// 0 to ok the move, or 1 to abort it.
//
UBOOL ULevel::CheckEncroachment
(
	AActor*		Actor,
	FVector		TestLocation,
	FRotator	TestRotation,
	UBOOL		bTouchNotify
)
{
	guard(ULevel::CheckEncroachment);
	check(Actor);
	return hp2_rs_engine_levact_check_encroachment( &GHp2LevActServices, &GMem, Hash, this, Actor, &TestLocation, &TestRotation, bTouchNotify );
	unguard;
}

/*-----------------------------------------------------------------------------
	SinglePointCheck.
-----------------------------------------------------------------------------*/

//
// Check for nearest hit.
// Return 1 if no hit, 0 if hit.
//
UBOOL ULevel::SinglePointCheck
(
	FCheckResult&	Hit,
	FVector			Location,
	FVector			Extent,
	DWORD			ExtraNodeFlags,
	ALevelInfo*		Level,
	UBOOL			bActors
)
{
	guard(ULevel::SinglePointCheck);
	FMemMark Mark(GMem);
	UBOOL Result = hp2_rs_engine_levact_single_point_check( &GHp2LevActServices, &GMem, Hash, &Hit, Level, &Location, &Extent, ExtraNodeFlags, bActors ) != 0;
	Mark.Pop();
	return Result;
	unguard;
}

/*-----------------------------------------------------------------------------
	MultiPointCheck.
-----------------------------------------------------------------------------*/

FCheckResult* ULevel::MultiPointCheck( FMemStack& Mem, FVector Location, FVector Extent, DWORD ExtraNodeFlags, ALevelInfo* Level, UBOOL bActors )
{
	guard(ULevel::MultiPointCheck);
	return (FCheckResult*)hp2_rs_engine_levact_multi_point_check( &GHp2LevActServices, &GMem, Hash, &Mem, Level, &Location, &Extent, ExtraNodeFlags, bActors );
	unguard;
}

/*-----------------------------------------------------------------------------
	SingleLineCheck.
-----------------------------------------------------------------------------*/

//
// Trace a line and return the first hit actor (LevelInfo means hit the world geomtry).
//
UBOOL ULevel::SingleLineCheck
(
	FCheckResult&	Hit,
	AActor*			SourceActor,
	const FVector&	End,
	const FVector&	Start,
	DWORD           TraceFlags,
	FVector			Extent,
	BYTE			ExtraNodeFlags
)
{
	guard(ULevel::Trace);
	FMemMark Mark(GMem);
	UBOOL Result = hp2_rs_engine_levact_single_line_check( &GHp2LevActServices, &GMem, Hash, &Hit, SourceActor, (TraceFlags & TRACE_Level) ? GetLevelInfo() : NULL, &End, &Start, &Extent, TraceFlags, ExtraNodeFlags ) != 0;
	Mark.Pop();
	return Result;
	unguard;
}

/*-----------------------------------------------------------------------------
	MultiLineCheck.
-----------------------------------------------------------------------------*/

FCheckResult* ULevel::MultiLineCheck
(
	FMemStack&		Mem,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	UBOOL			bCheckActors,
	ALevelInfo*		LevelInfo,
	BYTE			ExtraNodeFlags
)
{
	guard(ULevel::MultiLineCheck);
	return (FCheckResult*)hp2_rs_engine_levact_multi_line_check( &GHp2LevActServices, &GMem, Hash, &Mem, LevelInfo, &End, &Start, &Extent, bCheckActors, ExtraNodeFlags );
	unguard;
}

/*-----------------------------------------------------------------------------
	ULevel zone functions.
-----------------------------------------------------------------------------*/

//
// Figure out which zone an actor is in, update the actor's iZone,
// and notify the actor of the zone change.  Skips the zone notification
// if the zone hasn't changed.
//
void ULevel::SetActorZone( AActor* Actor, UBOOL bTest, UBOOL bForceRefresh )
{
	guard(ULevel::SetActorZone);
	check(Actor);
	hp2_rs_engine_levact_set_actor_zone( &GHp2LevActServices, &GMem, Hash, this, Actor, bTest, bForceRefresh );
	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
