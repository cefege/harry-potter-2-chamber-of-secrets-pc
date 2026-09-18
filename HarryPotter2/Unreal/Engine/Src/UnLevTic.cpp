/*=============================================================================
	UnLevTic.cpp: Level timer tick function
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "UnMesh.h"

/*-----------------------------------------------------------------------------
	Helper classes.
-----------------------------------------------------------------------------*/

//
// Priority sortable list.
//
struct FActorPriority
{
	INT			    Priority;	// Update priority, higher = more important.
	AActor*			Actor;		// Actor.
	UActorChannel*	Channel;	// Actor channel.
	FActorPriority()
	{}
	FActorPriority( FVector& ViewPos, FVector& ViewDir, UNetConnection* InConnection, AActor* InActor )
	{
		guard(FActorPriority::FActorPriority);
		Actor       = InActor;
		Channel     = InConnection->ActorChannels.FindRef(Actor);
		FLOAT Time  = Channel ? (InConnection->Driver->Time - Channel->LastUpdateTime) : InConnection->Driver->SpawnPrioritySeconds;
		FLOAT Dot = 0.f;
		if ( Actor->bAlwaysRelevant )
			Dot = 0.f;
		else if ( InConnection->Actor->Weapon == Actor )
			Dot = 1.f;
		else if ( (Actor->RemoteRole > ROLE_DumbProxy) || (Actor->Physics == PHYS_None) )
		{

			FVector Dir = Actor->Location - ViewPos;
			Dot   = ViewDir | Dir;
			if ( Dot < 0.f )
				Dot = -1.f;
			else
			{
				Dir = Dir.SafeNormal();
				Dot   = ViewDir | Dir;
			}
		}
		Priority    = appRound(65536.0f * (3.0f+Dot) * Actor->GetNetPriority( (Channel && Channel->Recent.Num()) ? (AActor*)&Channel->Recent(0) : NULL, Time, InConnection->BestLag ));
		if( InActor->bNetOptional )
			Priority -= 3000000;
		unguard;
	}
	friend INT Compare( const FActorPriority* A, const FActorPriority* B )
	{
		return B->Priority - A->Priority;
	}
};

/*-----------------------------------------------------------------------------
	Tick a single actor.
-----------------------------------------------------------------------------*/

UBOOL AActor::Tick( FLOAT DeltaSeconds, ELevelTick TickType )
{
	guard(AActor::Tick);

	//First, if TickParent is not null, Tick that actor
	if( TickParent )
		TickParent->Tick( DeltaSeconds, TickType );

	if( TickParent2 )
		TickParent2->Tick( DeltaSeconds, TickType );
	
	// Now, if Owner is not null, and owner->TickParent!=this, Handle owner-first updating.
	if( Owner  &&  Owner->TickParent != this  &&  Owner->TickParent2 != this )
		Owner->Tick( DeltaSeconds, TickType );

	if( (INT)bTicked==GetLevel()->Ticked )
		return 1;

	// If we are in special pause mode, disable all actors apart form those which 
	// have the appropriate flag set
	if (GetLevel()->bInSpecialPauseMode)
	{
		// if we are the player then get the input for the camera
		APlayerPawn* PlayerPawn = NULL;
		PlayerPawn = Cast<APlayerPawn>(this);
		if( PlayerPawn && PlayerPawn->Player )
		{
			// Continue to get player input
			PlayerPawn->Player->ReadInput( DeltaSeconds );
			PlayerPawn->eventPlayerInput( DeltaSeconds );
			PlayerPawn->Player->ReadInput( -1.0f );
		}

		if (!bCanMoveInSpecialPause)
			return 1;
		else
			bInSpecialPause = true;
	}
	else
	{
		bInSpecialPause = false;
	}

	// Ignore actors in stasis
	if
	(	bStasis 
	&&	(bForceStasis || (Physics==PHYS_None) || (Physics == PHYS_Rotating))
	&&	(GetLevel()->TimeSeconds-GetLevel()->Model->Zones[Region.ZoneNumber].LastRenderTime > 5)
	&&	(Level->NetMode == NM_Standalone) )
		return 1;

	bTicked = GetLevel()->Ticked;
	APawn* Pawn = NULL;
	if( bIsPawn )
		Pawn = Cast<APawn>(this);

	INT bSimulatedPawn = ( Pawn && (Role == ROLE_SimulatedProxy) );

	// Find actual actor which is animating.
	AActor* Animator = AnimBone != 0 && bAnimTransient ? Owner : this;
	check( Animator );

	// Update all animation, including multiple passes if necessary.
	INT Iterations = 0;
	FLOAT Seconds = DeltaSeconds;
	//if ( bSimulatedPawn )
	//	debugf("Animation %s frame %f rate %f tween %f",*AnimSequence,AnimFrame, AnimRate, TweenRate);
	//milestone 3 addition
	 if(GIsEditor)
	 {
		if( TweenRate > 0.0f )
		{

			TweenAlpha += TweenRate * Seconds;
			if( TweenAlpha >= 1.0f )
			{
				// Finished tweening.
				TweenAlpha = 0.0f;
				TweenRate = 0.0f;
				if( AnimRate == 0.0f )
				{
					bAnimFinished = 1;
					if ( !Animator->bSimulatedPawn )
						Animator->eventAnimEnd();
				}
			}
		}


	 }
	 else
	 {

	while
	(	IsAnimating()
	&&	(Seconds>0.0f)
	&&	(++Iterations <= 4) )
	{
		
		// Update tweening.
		if( TweenRate > 0.0f )
		{

			TweenAlpha += TweenRate * Seconds;
			if( TweenAlpha >= 1.0f )
			{
				// Finished tweening.
				TweenAlpha = 1.0f;
				TweenRate = 0.0f;
				if( AnimRate == 0.0f )
				{
					bAnimFinished = 1;
					if ( !Animator->bSimulatedPawn )
						Animator->eventAnimEnd();
				}
			}
		}

		// Remember the old frame.
		FLOAT OldAnimFrame = AnimFrame;

		// Update animation, and possibly overflow it.
		if( AnimRate != 0.0f )
		{
			// Update regular or velocity-scaled animation.
			if( AnimRate >= 0.0f )
				AnimFrame += AnimRate * Seconds;
			else
				AnimFrame += ::Max( AnimMinRate, Animator->Velocity.Size() * -AnimRate ) * Seconds;

			// Handle all animation sequence notifys.
			if( bAnimNotify && Mesh )
			{				
				const FMeshAnimSeq* Seq = GetAnim( AnimSequence );
				if( Seq )
				{
					FLOAT BestElapsedFrames = 100000.0f;
					const FMeshAnimNotify* BestNotify = NULL;
					for( INT i=0; i<Seq->Notifys.Num(); i++ )
					{
						const FMeshAnimNotify& Notify = Seq->Notifys(i);
						if( OldAnimFrame<Notify.Time && AnimFrame>=Notify.Time )
						{
							FLOAT ElapsedFrames = Notify.Time - OldAnimFrame;
							if( BestNotify==NULL || ElapsedFrames<BestElapsedFrames )
							{
								BestElapsedFrames = ElapsedFrames;
								BestNotify        = &Notify;
							}
						}
					}
					if( BestNotify )
					{
						Seconds   = Seconds * (AnimFrame - BestNotify->Time) / (AnimFrame - OldAnimFrame);
						AnimFrame = BestNotify->Time;
						UFunction* Function = Animator->FindFunction( BestNotify->Function );
						if( Function )
							Animator->ProcessEvent( Function, NULL );
						continue;
					}
				}
			}

			// Handle end of animation sequence.
			if( AnimFrame<AnimLast )
			{
				// We have finished the animation updating for this tick.
				break;
			}
			else if( bAnimLoop )
			{
				if( AnimFrame < 1.0f )
				{
					// Still looping.
					Seconds = 0.0f;
				}
				else
				{
					// Just passed end, so loop it.
					Seconds = Seconds * (AnimFrame - 1.0f) / (AnimFrame - OldAnimFrame);
					AnimFrame = 0.0f;
				}
				if( OldAnimFrame < AnimLast )
				{
					if( GetStateFrame()->LatentAction == EPOLL_FinishAnim )
						bAnimFinished = 1;
					if( !bSimulatedPawn )
						eventAnimEnd();
				}
			}
			else 
			{
				// Just passed end-minus-one frame.
				Seconds = Seconds * (AnimFrame - AnimLast) / (AnimFrame - OldAnimFrame);
				AnimFrame	 = AnimLast;
				bAnimFinished = 1;
				AnimRate      = 0.0f;
				if ( !Animator->bSimulatedPawn )
					Animator->eventAnimEnd();
				
				if ( (RemoteRole < ROLE_SimulatedProxy) && !IsA(AWeapon::StaticClass()) )
				{
					SimAnim.X = 10000 * AnimFrame;
					SimAnim.Y = 5000 * AnimRate;
					if ( SimAnim.Y > 32767 )
						SimAnim.Y = 32767;
				}
			}
		}
	}
	}

	// This actor is tickable.
	if( bSimulatedPawn )
	{
		// FIXME - predict fall for all pawns (COOP) - but need
		// new replicated bool for pawns which don't fly but don't fall
		// (i.e. stuck on wall, PHYS_Spider, etc.)
		if ( Pawn->bIsPlayer && !Pawn->bCanFly && !Region.Zone->bWaterZone )
		{
			// only add gravity if pawn is not resting on valid floor
			FCheckResult Hit(1.0f);
			GetLevel()->SingleLineCheck(Hit, this, Location - FVector(0,0,6), Location, TRACE_VisBlocking, GetCylinderExtent());
			if ( (Hit.Time == 1.0f) || (Hit.Normal.Z < 0.7f) )
			{
				if ( Velocity.Z == 0.f )
					Velocity.Z = -120.f;
				Velocity += 0.5f * Region.Zone->ZoneGravity * DeltaSeconds;
			}
		}
		//simulated pawns just predict location, no script execution
		moveSmooth(Velocity * DeltaSeconds);

		// Tick the nonplayer.
		if ( IsProbing(NAME_Tick) )
			eventTick(DeltaSeconds);
	}
	else if( RemoteRole == ROLE_AutonomousProxy ) 
	{
		if( Role == ROLE_Authority )
		{
			// update viewtarget replicated info
			APlayerPawn* PlayerPawn = NULL;
			if( Pawn )
			{
				PlayerPawn = Cast<APlayerPawn>(this);
			}
			if( PlayerPawn && PlayerPawn->ViewTarget )
			{
				APawn* TargetPawn = Cast<APawn>(PlayerPawn->ViewTarget);
				if ( TargetPawn )
				{
					PlayerPawn->TargetViewRotation = TargetPawn->ViewRotation;
					PlayerPawn->TargetEyeHeight = TargetPawn->EyeHeight;
					if ( TargetPawn->Weapon )
						PlayerPawn->TargetWeaponViewOffset = TargetPawn->Weapon->PlayerViewOffset;
				}
			}

			// Server handles timers for autonomous proxy.
			if( (TimerRate>0.0f) && (TimerCounter+=DeltaSeconds)>=TimerRate )
			{
				// Normalize the timer count.
				INT TimerTicksPassed = 1;
				if( TimerRate > 0.0f )
				{
					TimerTicksPassed  = appRound(TimerCounter/TimerRate);
					TimerCounter     -= TimerRate * TimerTicksPassed;
					if( TimerTicksPassed && !bTimerLoop )
					{
						// Only want a one-shot timer message.
						TimerTicksPassed = 1;
						TimerRate = 0.0f;
					}
				}

				// Call timer routine with count of timer events that have passed.
				eventTimer();
			}
		}
	}
	else if( Role>=ROLE_SimulatedProxy )
	{
		APlayerPawn* PlayerPawn = NULL;
		if ( Pawn )
			PlayerPawn = Cast<APlayerPawn>(this);
		if( !PlayerPawn || !PlayerPawn->Player )
		{
			// Non-player update.
			if( TickType==LEVELTICK_ViewportsOnly )
				return 1;

			// Tick the nonplayer.
			if ( IsProbing(NAME_Tick) )
				eventTick(DeltaSeconds);
		}
		else
		{
			// Player update.
			if( PlayerPawn->IsA(ACamera::StaticClass()) && !(PlayerPawn->ShowFlags & SHOW_PlayerCtrl) )
				return 1;

			// Process PlayerTick with input.
			PlayerPawn->Player->ReadInput( DeltaSeconds );
			PlayerPawn->eventPlayerInput( DeltaSeconds );
			PlayerPawn->eventPlayerTick( DeltaSeconds );
			PlayerPawn->Player->ReadInput( -1.0f );

			if( GetLevel()->DemoRecDriver && !GetLevel()->DemoRecDriver->ServerConnection )
			{
				PlayerPawn->DemoViewPitch = PlayerPawn->ViewRotation.Pitch;
				PlayerPawn->DemoViewYaw = PlayerPawn->ViewRotation.Yaw;
			}
		}

		// Update the actor's script state code.
		ProcessState( DeltaSeconds );

		// Update timers.
		if( TimerRate>0.0f && (TimerCounter+=DeltaSeconds)>=TimerRate )
		{
			// Normalize the timer count.
			INT TimerTicksPassed = 1;
			if( TimerRate > 0.0f )
			{
				TimerTicksPassed  = appRound(TimerCounter/TimerRate);
				TimerCounter     -= TimerRate * TimerTicksPassed;
				if( TimerTicksPassed && !bTimerLoop )
				{
					// Only want a one-shot timer message.
					TimerTicksPassed = 1;
					TimerRate = 0.0f;
				}
			}

			// Call timer routine with count of timer events that have passed.
			eventTimer();
		}

		// Update LifeSpan.
		if( LifeSpan!=0.f )
		{
			LifeSpan -= DeltaSeconds;
			if( LifeSpan <= 0.0001f )
			{
				// Actor's LifeSpan expired.
				eventExpired();
				GetLevel()->DestroyActor( this );
				return 1;
			}
		}

		// Perform physics.
		if( Physics!=PHYS_None && Role!=ROLE_AutonomousProxy )
			performPhysics( DeltaSeconds );
	}
	else if ( Physics == PHYS_Falling ) // dumbproxies simulate falling if client side physics set
		performPhysics( DeltaSeconds );

	// During demo playback, setup view offsets for viewtarget
	if( GetLevel()->DemoRecDriver && GetLevel()->DemoRecDriver->ServerConnection )
	{
		if( Role == ROLE_Authority )
		{
			// update viewtarget replicated info
			APlayerPawn* PlayerPawn = NULL;
			if( Pawn )
			{
				PlayerPawn = Cast<APlayerPawn>(this);
			}
			if( PlayerPawn && PlayerPawn->ViewTarget && !PlayerPawn->bBehindView )
			{
				APawn* TargetPawn = Cast<APawn>(PlayerPawn->ViewTarget);
				if ( TargetPawn )
				{
					PlayerPawn->TargetViewRotation = TargetPawn->ViewRotation;
					PlayerPawn->TargetEyeHeight = TargetPawn->EyeHeight;
					if ( TargetPawn->Weapon )
						PlayerPawn->TargetWeaponViewOffset = TargetPawn->Weapon->PlayerViewOffset;
				}
			}
		}
	}
	
	// Update eyeheight and send visibility updates
	// with PVS, monsters look for other monsters, rather than sending msgs
	// Also sends PainTimer messages if PainTime
	if( Pawn )
	{
		if( Pawn->bIsPlayer && Role>=ROLE_AutonomousProxy )
		{
			if ( Pawn->bViewTarget )
				Pawn->eventUpdateEyeHeight( DeltaSeconds );
			else
				Pawn->ViewRotation = Rotation;
		}

		// update weapon location (in case its playing sounds, etc.)
		if ( Pawn->Weapon )
		{
			GetLevel()->FarMoveActor( Pawn->Weapon, Location );
		}
		if( Role==ROLE_Authority && TickType==LEVELTICK_All )
		{
			if( Pawn->SightCounter < 0.0f )
			{
				// reset SightCounter  after one full tick negative so that any player pawns' ShowSelf() will trigger this pawn
				Pawn->SightCounter += 0.2f;
			}
			Pawn->SightCounter = Pawn->SightCounter - DeltaSeconds; 
			if( Pawn->bIsPlayer && !Pawn->bHidden )
			{
				// players (bots and playerpawns) show themselves every tick to
				// any other pawns who are probing the event SeePlayer() and their sight counter is currently < 0
				Pawn->ShowSelf();
			}
			if( Pawn->SightCounter<0.0f && Pawn->IsProbing(NAME_EnemyNotVisible) )
			{
				// if pawn currently has an enemy, check that enemy is visible 
				// do this check every 0.1 seconds
				Pawn->CheckEnemyVisible();
				Pawn->SightCounter = 0.1f;
			}
			if( Pawn->PainTime > 0.0f )
			{
				Pawn->PainTime -= DeltaSeconds;
				if (Pawn->PainTime < 0.001f)
				{
					Pawn->PainTime = 0.0f;
					Pawn->eventPainTimer();
				}
			}
			if( Pawn->SpeechTime > 0.0f )
			{
				Pawn->SpeechTime -= DeltaSeconds;
				if (Pawn->SpeechTime < 0.001)
				{
					Pawn->SpeechTime = 0.0f;
					Pawn->eventSpeechTimer();
				}
			}
			if ( Pawn->bAdvancedTactics )
				Pawn->eventUpdateTactics(DeltaSeconds);
		}
	}

	// Perform animation-to-movement conversion if requested.
	if( bAnimMove && Role==ROLE_Authority && Mesh )
	{
		FCheckResult Hit(1.f);
		FVector Move = Mesh->GetRootMovement(this);
		if( !Move.IsZero() )
		{
			GetLevel()->MoveActor( this, Move, Rotation, Hit );
			Move *= 1.f - Hit.Time;
			if( Hit.Time < 1.f )
			{
				// Slide along surface.
				FVector Move2 = Move - Hit.Normal * (Move | Hit.Normal);
				GetLevel()->MoveActor( this, Move2, Rotation, Hit );
				Move -= Move2 * Hit.Time;
			}
			Mesh->AdjustRootMovement(this, Move);
		}
	}

	return 1;
	unguard;
}

/*-----------------------------------------------------------------------------
	Network client tick.
-----------------------------------------------------------------------------*/

void ULevel::TickNetClient( FLOAT DeltaSeconds )
{
	guard(ULevel::TickNetClient);
	clock(NetTickCycles);
	if( NetDriver->ServerConnection->State==USOCK_Open )
	{
		for( TMap<AActor*,UActorChannel*>::TIterator ItC(NetDriver->ServerConnection->ActorChannels); ItC; ++ItC )
		{
			guard(UpdateLocalActors);
			UActorChannel* It = ItC.Value();
			APlayerPawn* PlayerPawn = Cast<APlayerPawn>(It->GetActor());
			if( PlayerPawn && PlayerPawn->Player )
				It->ReplicateActor();
			unguard;
		}
	}
	else if( NetDriver->ServerConnection->State==USOCK_Closed )
	{
		// Server disconnected.
		check(Engine->Client->Viewports.Num());
		Engine->SetClientTravel( Engine->Client->Viewports(0), TEXT("?failed"), 0, TRAVEL_Absolute );
	}
	unclock(NetTickCycles);
	unguard;
}

/*-----------------------------------------------------------------------------
	Network server ticking individual client.
-----------------------------------------------------------------------------*/

UBOOL ActorCanSee( AActor* Actor, APlayerPawn* RealViewer, AActor* Viewer, FVector SrcLocation )
{
	guardSlow(ActorCanSee);
	if( Actor->bAlwaysRelevant || Actor->IsOwnedBy(Viewer) || Actor->IsOwnedBy(RealViewer) || Actor==Viewer || Actor==RealViewer
		|| Viewer==Actor->Instigator )
		return 1;
	else if( Actor->AmbientSound 
			&& ((Actor->Location-Viewer->Location).SizeSquared() < 0.3*Actor->WorldSoundRadius()*Actor->WorldSoundRadius()) )
		return 1;
	else if( Actor->Owner && Actor->Owner->bIsPawn && Actor==((APawn*)Actor->Owner)->Weapon )
		return ActorCanSee( Actor->Owner, RealViewer, Viewer, SrcLocation );
	else if( (Actor->bHidden || Actor->bOnlyOwnerSee) && !Actor->bBlockPlayers && !Actor->AmbientSound )
		return 0;
	else
		return Actor->GetLevel()->Model->FastLineCheck(Actor->Location,SrcLocation);
	unguardSlow;
}

INT ULevel::ServerTickClient( UNetConnection* Connection, FLOAT DeltaSeconds )
{
	guard(ULevel::ServerTickClient);
	check(Connection);
	check(Connection->State==USOCK_Pending || Connection->State==USOCK_Open || Connection->State==USOCK_Closed);
	//FTime CullTime=0.0, TraceTime=0.0, RepTime=0.0f; INT CullCount=0, RepCount=0;
	//INT TempCull = 0;

	// Handle not ready channels.
	INT Updated=0;
	if( Connection->Actor && Connection->IsNetReady(0) && Connection->State==USOCK_Open 
		&& Connection->Driver->Time-Connection->LastReceiveTime<1.5f )
	{
		// Get list of visible/relevant actors.
		FMemMark Mark(GMem);
		NetTag++;
		Connection->TickCount++;

		// Set up to skip all sent temporary actors.
		guard(SkipSentTemporaries);
		for( INT i=0; i<Connection->SentTemporaries.Num(); i++ )
			Connection->SentTemporaries(i)->NetTag = NetTag;
		unguard;

		// Get viewer coordinates.
		AActor*      Viewer    = Connection->Actor;
		APlayerPawn* InViewer  = Connection->Actor;
		FVector      Location  = InViewer->Location;
		FRotator     Rotation  = InViewer->ViewRotation;
		InViewer->eventPlayerCalcView( Viewer, Location, Rotation );
		check(Viewer);

		// Compute ahead-vectors for prediction.
		FVector Ahead = FVector(0,0,0);
		if( Connection->TickCount & 1 )
		{
			FLOAT PredictSeconds = (Connection->TickCount&2) ? 0.4f : 0.9f;
			Ahead = PredictSeconds * Viewer->Velocity;
			if( Viewer->Base )
				Ahead += PredictSeconds * Viewer->Base->Velocity;
			FCheckResult Hit(1.0f);
			Hit.Location = Location + Ahead;
			Viewer->GetLevel()->Model->LineCheck(Hit,NULL,Hit.Location,Location,FVector(0,0,0),NF_NotVisBlocking);
			Location = Hit.Location;
		}

		// Make list of all actors to consider.
		//CullTime-=appSeconds();
		INT              ConsiderCount  = 0;
		FActorPriority*  PriorityList   = new(GMem,Actors.Num())FActorPriority;
		FActorPriority** PriorityActors = new(GMem,Actors.Num())FActorPriority*;
		FVector          ViewPos        = Viewer->Location;
		FVector          ViewDir        = InViewer->ViewRotation.Vector();
		FTime			 LastTime		= Connection->LastRepTime;
		FTime            ThisTime       = Connection->Driver->Time;
		guard(MakeConsiderList);

		// add LevelInfo to the list
		for( INT i=0; i<2; i++ )
		{
			AActor* Actor = Actors(i);
			if( Actor && (Actor->NetTag!=NetTag)
				&&	(Actor->RemoteRole!=ROLE_None) )
			{
				//CullCount++;
				Actor->NetTag                 = NetTag;
				PriorityList  [ConsiderCount] = FActorPriority( ViewPos, ViewDir, Connection, Actor );
				PriorityActors[ConsiderCount] = PriorityList + ConsiderCount++;
			}
		}
		FLOAT ServerTickRate = Engine->GetMaxTickRate();
		for( INT i=iFirstNetRelevantActor; i<Actors.Num(); i++ )
		{
			AActor* Actor = Actors(i);
			if( Actor 
				&& (Actor->NetTag!=NetTag)
				&&	(Actor->RemoteRole!=ROLE_None) )
			{
				if ( Actor->bAlwaysRelevant )
				{
					if( appRound(LastTime.GetFloat()*Actor->NetUpdateFrequency)!=appRound(ThisTime.GetFloat()*Actor->NetUpdateFrequency) )
					{
						UActorChannel* Channel = NULL;
						if ( Actor->CheckRecentChanges() )
							Channel = Connection->ActorChannels.FindRef(Actor);
						if ( Channel 
							&& Channel->Recent.Num() 
							&& Channel->Dirty.Num() == 0
							&& Actor->NoVariablesToReplicate((AActor*)&(Channel->Recent(0))) )
						{
							//TempCull++;
							Channel->RelevantTime = NetDriver->Time;
						}
						else
						{
							//CullCount++;
							Actor->NetTag                 = NetTag;
							PriorityList  [ConsiderCount] = FActorPriority( ViewPos, ViewDir, Connection, Actor );
							PriorityActors[ConsiderCount] = PriorityList + ConsiderCount++;
						}
					}
					LastTime = LastTime + 0.023f;
					ThisTime = ThisTime + 0.023f;
				}
				else if ( !Actor->bNetOptional 
					|| (Actor->LifeSpan > Actor->GetClass()->GetDefaultActor()->LifeSpan - 0.15f) )
				{
					FLOAT UpdateFreq = Actor->UpdateFrequency(Viewer, ViewDir, ViewPos); 

					if( (UpdateFreq >= ServerTickRate)
						|| (appRound(LastTime.GetFloat()*UpdateFreq)!=appRound(ThisTime.GetFloat()*UpdateFreq)) )
					{
						//CullCount++;
						Actor->NetTag                 = NetTag;
						PriorityList  [ConsiderCount] = FActorPriority( ViewPos, ViewDir, Connection, Actor );
						PriorityActors[ConsiderCount] = PriorityList + ConsiderCount++;
					}
					LastTime = LastTime + 0.023f;
					ThisTime = ThisTime + 0.023f;
				}
			}
		}
		Connection->LastRepTime = Connection->Driver->Time;
		unguard;

		// Sort by priority.
		guard(SortConsiderList);
		Sort( PriorityActors, ConsiderCount );
		//CullTime+=appSeconds();
		unguard;

		// Update all relevant actors in sorted order.
		guard(UpdateRelevant);
		for( INT j=0; j<ConsiderCount; j++ )
		{
			AActor*        Actor       = PriorityActors[j]->Actor;
			UActorChannel* Channel     = PriorityActors[j]->Channel;
			//TraceTime-=appSeconds();
			UBOOL          CanSee      = 0;
			// only check visibility on already visible actors every 0.3 + 0.2R seconds
			if ( !Channel || NetDriver->Time-Channel->RelevantTime>0.3 )
				CanSee = ActorCanSee( Actor, InViewer, Viewer, Location );
			//TraceTime+=appSeconds();
			if( CanSee || (Channel && NetDriver->Time-Channel->RelevantTime<NetDriver->RelevantTimeout) )
			{
				// Find or create the channel for this actor.
				Actor->GetLevel()->NumPV++;
				if( !Channel && Connection->PackageMap->ObjectToIndex(Actor->GetClass())!=INDEX_NONE )
				{
					// Create a new channel for this actor.
					Channel = (UActorChannel*)Connection->CreateChannel( CHTYPE_Actor, 1 );
					if( Channel )
						Channel->SetChannelActor( Actor );
				}
				if( Channel )
				{
					if ( !Connection->IsNetReady(0) ) // here also???
						break;
					if( CanSee )
						Channel->RelevantTime = NetDriver->Time + 0.2f * appFrand();
					if( Channel->IsNetReady(0) )
					{
						//debugf(TEXT("Replicate %s priority %d"), Actor->GetName(), PriorityActors[j]->Priority);
						//RepTime-=appSeconds();
						//RepCount++;
						Channel->ReplicateActor();
						//RepTime+=appSeconds();
						Updated++;
					}
					if ( !Connection->IsNetReady(0) )
						break;
				}
			}
			else if( Channel )
				Channel->Close();
		}
		unguard;
		Mark.Pop();
	}
	//if( NetDriver->ProfileStats )
	//	debugf(TEXT("Actors %04i AlwaysRel %03i Cull=%01.4f (%03i) Trace=%01.4f Rep=%01.4f (%03i)"),Actors.Num(), TempCull, CullTime*1000,CullCount,TraceTime*1000,RepTime*1000,RepCount);
	return Updated;
	unguard;
}

/*-----------------------------------------------------------------------------
	Network server tick.
-----------------------------------------------------------------------------*/

void ULevel::TickNetServer( FLOAT DeltaSeconds )
{
	guard(ULevel::TickNetServer);

	// Update all clients.
	clock(NetTickCycles);
	INT Updated=0;
	for( INT i=NetDriver->ClientConnections.Num()-1; i>=0; i-- )
		Updated += ServerTickClient( NetDriver->ClientConnections(i), DeltaSeconds );
	unclock(NetTickCycles);

	// Log message.
	if( appRound(TimeSeconds-DeltaSeconds)!=appRound(TimeSeconds.GetFloat()) )
		debugf( NAME_Title, LocalizeProgress("RunningNet"), *GetLevelInfo()->Title, *URL.Map, NetDriver->ClientConnections.Num() );

	// Stats.
	if( Updated )
	{
		for( INT i=0; i<NetDriver->ClientConnections.Num(); i++ )
		{
			UNetConnection* Connection = NetDriver->ClientConnections(i);
			if( Connection->Actor && Connection->State==USOCK_Open )
			{
				if( Connection->UserFlags&1 )
				{
					// Send stats.
					INT NumActors=0;
					for( INT i=0; i<Actors.Num(); i++ )
						NumActors += Actors(i)!=NULL;
					FString Stats = FString::Printf
					(
						TEXT("r=%i cli=%i act=%03.1f (%i) net=%03.1f pv/c=%i rep/c=%i rpc/c=%i"),
						appRound(Engine->GetMaxTickRate()),
						NetDriver->ClientConnections.Num(),
						GSecondsPerCycle*1000*ActorTickCycles,
						NumActors,
						GSecondsPerCycle*1000*NetTickCycles,
						NumPV  /NetDriver->ClientConnections.Num(),
						NumReps/NetDriver->ClientConnections.Num(),
						NumRPC /NetDriver->ClientConnections.Num()
					);
					Connection->Actor->eventClientMessage( *Stats, NAME_None, 0 );
				}
				if( Connection->UserFlags&2 )
				{
					FString Stats = FString::Printf
					(
						TEXT("snd=%02.1f recv=%02.1f"),
						GSecondsPerCycle*1000*Connection->Driver->SendCycles,
						GSecondsPerCycle*1000*Connection->Driver->RecvCycles
					);
					Connection->Actor->eventClientMessage( *Stats, NAME_None, 0 );
				}
			}
		}
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Demo Recording tick.
-----------------------------------------------------------------------------*/

INT ULevel::TickDemoRecord( FLOAT DeltaSeconds )
{
	guard(ULevel::TickDemo);

	// All replicatable actors are assumed to be relevant for demo recording.
	UNetConnection* Connection = DemoRecDriver->ClientConnections(0);
	for( INT i=0; i<Actors.Num(); i++ )
	{
		AActor* Actor = Actors(i);
		UBOOL IsNetClient = (GetLevelInfo()->NetMode == NM_Client);
		if
		(	Actor
		&&	(Actor->RemoteRole!=ROLE_None || (IsNetClient && Actor->Role!=ROLE_None && Actor->Role != ROLE_Authority))
		&&  (i>=iFirstDynamicActor || Actor->IsA(AZoneInfo::StaticClass()))
		&&  (!Actor->bNetTemporary || Connection->SentTemporaries.FindItemIndex(Actor)==INDEX_NONE)
		&&  (Actor->bStatic || !Actor->GetClass()->GetDefaultActor()->bStatic))
		{
			// Create a new channel for this actor.
			UActorChannel* Channel = Connection->ActorChannels.FindRef( Actor );
			if( !Channel && Connection->PackageMap->ObjectToIndex(Actor->GetClass())!=INDEX_NONE )
			{
				// Check we haven't run out of actor channels.
				Channel = (UActorChannel*)Connection->CreateChannel( CHTYPE_Actor, 1 );
				check(Channel);
				Channel->SetChannelActor( Actor );
			}
			if( Channel )
			{
				// Send it out!
				check(!Channel->Closing);
				if( Channel->IsNetReady(0) )
				{
					Actor->bDemoRecording = 1;
					Actor->bClientDemoRecording = IsNetClient;
					if(IsNetClient)
						Exchange(Actor->RemoteRole, Actor->Role);
					Channel->ReplicateActor();
					if(IsNetClient)
						Exchange(Actor->RemoteRole, Actor->Role);
					Actor->bDemoRecording = 0;
					Actor->bClientDemoRecording = 0;
				}
			}
		}
	}
	return 1;
	unguard;
}
INT ULevel::TickDemoPlayback( FLOAT DeltaSeconds )
{
	guard(ULevel::TickDemoPlayback);
	if
	(	GetLevelInfo()->LevelAction==LEVACT_Connecting 
	&&	DemoRecDriver->ServerConnection->State!=USOCK_Pending )
	{
		GetLevelInfo()->LevelAction = LEVACT_None;
		Engine->SetProgress( TEXT(""), TEXT(""), 0.0f );
	} 
	if( DemoRecDriver->ServerConnection->State==USOCK_Closed )
	{
		// Demo stopped playing
		check(Engine->Client->Viewports.Num());
		Engine->SetClientTravel( Engine->Client->Viewports(0), TEXT("?entry"), 0, TRAVEL_Absolute );
	}
	return 1;
	unguard;
}

/*-----------------------------------------------------------------------------
	Main level timer tick handler.
-----------------------------------------------------------------------------*/

//
// Update the level after a variable amount of time, DeltaSeconds, has passed.
// All child actors are ticked after their owners have been ticked.
//
void ULevel::Tick( ELevelTick TickType, FLOAT DeltaSeconds )
{
	guard(ULevel::Tick);
	ALevelInfo* Info = GetLevelInfo();
	InitStats();
	FMemMark Mark(GMem);
	FMemMark EngineMark(GEngineMem);
	GInitRunaway();
	InTick=1;

	//Keep actor time profile FIXME TEMP!!!
	Info->AvgAITime = 0.95f * GetLevelInfo()->AvgAITime + 0.05f * 1000.f * GSecondsPerCycle * ActorTickCycles;
	Info->AIProfile[Clamp(appRound(10 * GSecondsPerCycle * ActorTickCycles/DeltaSeconds), 0, 7)] += 1;

	// Update the net code and fetch all incoming packets.
	guard(UpdatePreNet);
	if( NetDriver )
	{
		NetDriver->TickDispatch( DeltaSeconds );
		if( NetDriver->ServerConnection )
			TickNetClient( DeltaSeconds );
	}
	unguard;

	// Fetch demo playback packets from demo file.
	guard(UpdatePreDemoRec);
	if( DemoRecDriver )
	{
		DemoRecDriver->TickDispatch( DeltaSeconds );
		if( DemoRecDriver->ServerConnection )
			TickDemoPlayback( DeltaSeconds );
	}
	unguard;

	// Update collision.
	guard(UpdateCollision);
	if( Hash )
		Hash->Tick();
	unguard;

	// Update time.
	guard(UpdateTime);

	// Clamp time between 200 fps and 10 fps.
	DeltaSeconds = Clamp(DeltaSeconds,0.005f,0.10f);
	DeltaSeconds *= Info->TimeDilation;
	TimeSeconds = TimeSeconds + DeltaSeconds;
	Info->TimeSeconds = TimeSeconds.GetFloat();
	UpdateTime(Info);
	if( Info->bPlayersOnly )
		TickType = LEVELTICK_ViewportsOnly;
	unguard;

	// If caller wants time update only, or we are paused, skip the rest.
	clock(ActorTickCycles);
	if
	(	(TickType!=LEVELTICK_TimeOnly)
	&&	Info->Pauser==TEXT("")
	&&	(!NetDriver || !NetDriver->ServerConnection || NetDriver->ServerConnection->State==USOCK_Open) )
	{
		// Tick all actors, owners before owned.
		guard(TickAllActors);
		NewlySpawned = NULL;
		INT Updated  = 0;
		for( INT iActor=iFirstDynamicActor; iActor<Actors.Num(); iActor++ )
			if( Actors( iActor ) )
				Updated += Actors( iActor )->Tick(DeltaSeconds,TickType);
		while( NewlySpawned && Updated )
		{
			FActorLink* Link = NewlySpawned;
			NewlySpawned     = NULL;
			Updated          = 0;
			for( Link; Link; Link=Link->Next )
				if( Link->Actor->bTicked!=(DWORD)Ticked )
					Updated += Link->Actor->Tick( DeltaSeconds, TickType );
		}
		unguard;
	}
	else if( Info->Pauser!=TEXT("") )
	{
		// Absorb input if paused.
		guard(AbsorbedPaused);
		for( INT iActor=iFirstDynamicActor; iActor<Actors.Num(); iActor++ )
		{
			APlayerPawn* PlayerPawn=Cast<APlayerPawn>(Actors(iActor));
			if( PlayerPawn && PlayerPawn->Player )
			{
				PlayerPawn->Player->ReadInput( DeltaSeconds );
				PlayerPawn->eventPlayerInput( DeltaSeconds );
				for( TFieldIterator<UFloatProperty> It(PlayerPawn->GetClass()); It; ++It )
					if( It->PropertyFlags & CPF_Input )
						*(FLOAT*)((BYTE*)PlayerPawn + It->Offset) = 0.f;
			}
			else if( Actors(iActor) && Actors(iActor)->bAlwaysTick )
				Actors(iActor)->Tick(DeltaSeconds,TickType);
		}
		unguard;
	}
	unclock(ActorTickCycles);

	// Update net server and flush networking.
	guard(UpdateNetServer);
	if( NetDriver )
	{
		if( !NetDriver->ServerConnection )
			TickNetServer( DeltaSeconds );
		NetDriver->TickFlush();
	}
	unguard;

	// Demo Recording.
	guard(UpdatePostDemoRec);
	if( DemoRecDriver )
	{
		if( !DemoRecDriver->ServerConnection )
			TickDemoRecord( DeltaSeconds );
		DemoRecDriver->TickFlush();
	}
	unguard;

	// Finish up.
	Ticked = !Ticked;
	InTick = 0;
	Mark.Pop();
	EngineMark.Pop();
	CleanupDestroyed( 0 );

	unguardf(( TEXT("(NetMode=%i)"), GetLevelInfo()->NetMode ));
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
