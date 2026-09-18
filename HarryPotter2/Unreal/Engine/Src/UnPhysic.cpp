/*=============================================================================
	UnPhysic.cpp: Actor physics implementation

	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Steven Polge 3/97
=============================================================================*/

#include "EnginePrivate.h"
#include "UnMesh.h"

void AActor::execMoveSmooth( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execMoveSmooth);

	P_GET_VECTOR(Delta);
	P_FINISH;

	bJustTeleported = 0;
	int didHit = moveSmooth(Delta);

	*(DWORD*)Result = didHit;
	unguardexecSlow;
}

void AActor::execSetPhysics( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetPhysics);

	P_GET_BYTE(NewPhysics);
	P_FINISH;

	setPhysics(NewPhysics);

	unguardSlow;
}

void AActor::execAutonomousPhysics( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execAutonomousPhysics);

	P_GET_FLOAT(DeltaSeconds);
	P_FINISH;

	// round acceleration to be consistent with replicated acceleration
	Acceleration.X = 0.1f * appRound(10.f * Acceleration.X);
	Acceleration.Y = 0.1f * appRound(10.f * Acceleration.Y);
	Acceleration.Z = 0.1f * appRound(10.f * Acceleration.Z);

	// Perform physics.
	if( Physics!=PHYS_None )
		performPhysics( DeltaSeconds );

	unguardSlow;
}

//======================================================================================

int AActor::moveSmooth(FVector Delta)
{
	guard(AActor::moveSmooth);

	FCheckResult Hit(1.f);
	int didHit = GetLevel()->MoveActor( this, Delta, Rotation, Hit );
	if (Hit.Time < 1.f)
	{
		FVector Adjusted = (Delta - Hit.Normal * (Delta | Hit.Normal)) * (1.f - Hit.Time);

		if( (Delta | Adjusted) >= 0 )
		{
			FVector OldHitNormal = Hit.Normal;
			FVector DesiredDir = Delta.SafeNormal();

			//ft: moved eventHitWall here, so you always get a hitwall when you actually hit a wall.
			eventHitWall(Hit.Normal, Hit.Actor);

			GetLevel()->MoveActor(this, Adjusted, Rotation, Hit);

			if (Hit.Time < 1.f)
			{
				//eventHitWall(Hit.Normal, Hit.Actor);
				TwoWallAdjust(DesiredDir, Adjusted, Hit.Normal, OldHitNormal, Hit.Time);
				GetLevel()->MoveActor(this, Adjusted, Rotation, Hit);
			}
		}
	}

	return didHit;
	unguard;
}

void AActor::FindBase()
{
	guard(AActor::findBase);

	FCheckResult Hit(1.f);
	GetLevel()->SingleLineCheck( Hit, this, Location + FVector(0,0,-8), Location, TRACE_AllBlocking, GetCylinderExtent() );
	if (Base != Hit.Actor)
		SetBase(Hit.Actor);

	unguard;
}

void AActor::setPhysics(BYTE NewPhysics, AActor *NewFloor)
{
	guard(AActor::setPhysics);

	if (Physics == NewPhysics)
		return;
	Physics = NewPhysics;

	if ((Physics == PHYS_Walking) || (Physics == PHYS_None) || (Physics == PHYS_Rolling) 
			|| (Physics == PHYS_Rotating) || (Physics == PHYS_Spider) )
	{
		if (NewFloor != NULL)
		{
			if (Base != NewFloor)
				SetBase(NewFloor);
		}
		else
			FindBase();
	}
	else if (Base != NULL)
		SetBase(NULL);

	if ( (Physics == PHYS_None) || (Physics == PHYS_Rotating) )
	{
		Velocity = FVector(0,0,0);
		Acceleration = FVector(0,0,0);
	}
	unguard;
}

void AActor::performPhysics(FLOAT DeltaSeconds)
{
	guard(AActor::performPhysics);

	FVector OldVelocity = Velocity;

	// change position
	switch (Physics)
	{
		case PHYS_Projectile: physProjectile(DeltaSeconds, 0); break;
		case PHYS_Falling: physFalling(DeltaSeconds, 0); break;
		case PHYS_Rotating: break;
		case PHYS_Interpolating: 
			{
/*				OldLocation = Location;
				physPathing(DeltaSeconds); 
				Velocity = (Location - OldLocation)/DeltaSeconds; */
				break;
			}
		case PHYS_MovingBrush: 
			{
				OldLocation = Location;
				physMovingBrush(DeltaSeconds); 
				Velocity = (Location - OldLocation)/DeltaSeconds;
				break;
			}
		case PHYS_Trailer: physTrailer(DeltaSeconds); break;
		case PHYS_Rolling: physRolling(DeltaSeconds, 0); break;
	}

	// rotate
	if ( !RotationRate.IsZero() ) 
		physicsRotation(DeltaSeconds);

	// allow touched actors to impact physics
	if ( PendingTouch )
	{
		PendingTouch->eventPostTouch(this);
		AActor *OldTouch = PendingTouch;
		PendingTouch = PendingTouch->PendingTouch;
		OldTouch->PendingTouch = NULL;
	}
	unguard;
}

void StepUpDownStairs(APawn *const ap)
{
	float delta, epsilon = 1.0f;
	
	// do it just if pawn is WALKING up/down stairs
	if(ap->Physics == PHYS_Walking)
	{
		float deltaZ = (ap->Location - ap->OldLocation).Size2D() * ap->StepHeight / ap->StepWidth;
		if(deltaZ > ap->StepHeight)	
			deltaZ = ap->StepHeight;

		// if going UP ....
		if(ap->SavedPrePivotZ < 0.0f)
		{
			ap->SavedPrePivotZ += deltaZ;
			if(ap->SavedPrePivotZ > 0.0f)
				ap->SavedPrePivotZ = 0.0f;
		}

		// if going DOWN ....
		else if(ap->SavedPrePivotZ > 0.0f)
 		{
 			ap->SavedPrePivotZ -= deltaZ;
	 		if(ap->SavedPrePivotZ < 0.0f)
 				ap->SavedPrePivotZ = 0.0f;
	 	}

		delta = ap->OldLocation.Z - ap->Location.Z;
	
		// Start to go Down ...
		if((delta < ap->StepHeight + epsilon) && (delta > ap->StepHeight - epsilon))
		{
			ap->SavedPrePivotZ = ap->StepHeight - deltaZ;
		}

		delta = ap->Location.Z - ap->OldLocation.Z;

		// Start to go UP ...
		if((delta < ap->StepHeight + epsilon) && (delta > ap->StepHeight - epsilon))
		{
			ap->SavedPrePivotZ = -(ap->StepHeight - deltaZ);
		}
	}

	// if this is not a walking phisics anymore, reset PrePivot back
	else
	{
		ap->SavedPrePivotZ = 0.0f;
	}
}

void APawn::performPhysics(FLOAT DeltaSeconds)
{
	guard(APawn::performPhysics);

	FVector OldVelocity = Velocity;

	// change position
	switch (Physics)
	{
		case PHYS_Walking: physWalking(DeltaSeconds, 0); break;
		case PHYS_Falling: physFalling(DeltaSeconds, 0); break;
		case PHYS_Flying: physFlying(DeltaSeconds, 0); break;
		case PHYS_Swimming: physSwimming(DeltaSeconds, 0); break;
		case PHYS_Spider: physSpider(DeltaSeconds, 0); break;
		case PHYS_Interpolating: 
			{
		/*		OldLocation = Location;
				physPathing(DeltaSeconds); 
				Velocity = (Location - OldLocation)/DeltaSeconds;  pk */
				break;
			}
		case PHYS_Projectile: physProjectile(DeltaSeconds, 0); break;
		case PHYS_Trailer: physTrailer(DeltaSeconds); break;
		case PHYS_Rolling: physRolling(DeltaSeconds, 0); break;
	}

	// check it out if he is WALKING up (or down) stairs
	StepUpDownStairs(this);

	// rotate
	if(   (Physics != PHYS_Spider)
	   && (Physics != PHYS_Interpolating) //At this point in time, you dont do any rotations when you're interpolating.  This needs work...
	   && (   IsA(APlayerPawn::StaticClass())
	       || (Rotation != DesiredRotation  &&  /*Physics != PHYS_Trailer)//*/bRotateToDesired)
		   //|| (RotationRate.Roll > 0)     <=-- this must've been a hack for something
		   || !bRotateToDesired && bFixedRotationDir
	      )
	  ) 
		physicsRotation(DeltaSeconds, OldVelocity);

	MoveTimer -= DeltaSeconds;
	AvgPhysicsTime = 0.8f * AvgPhysicsTime + 0.2f * DeltaSeconds;

	if ( PendingTouch )
	{
		PendingTouch->eventPostTouch(this);
		if ( PendingTouch )
		{
			AActor *OldTouch = PendingTouch;
			PendingTouch = PendingTouch->PendingTouch;
			OldTouch->PendingTouch = NULL;
		}
	}

	unguard;
}

int AActor::fixedTurn(int current, int desired, int deltaRate)
{
	guard(AActor::fixedTurn);

	if (deltaRate == 0)
		return (current & 65535);

	int result = current & 65535;
	current = result;
	desired = desired & 65535;

	if (bFixedRotationDir)
	{
		if (bRotateToDesired)
		{
			if (deltaRate > 0)
			{
				if (current > desired)
					desired += 65536;
				result += Min(deltaRate, desired - current);
			}
			else 
			{
				if (current < desired)
					current += 65536;
				result += ::Max(deltaRate, desired - current);
			}
		}
		else
			result += deltaRate;
	}
	else if (bRotateToDesired)
	{
		if (current > desired)
		{
			if (current - desired < 32768)
				result -= Min((current - desired), Abs(deltaRate));
			else
				result += Min((desired + 65536 - current), Abs(deltaRate));
		}
		else
		{
			if (desired - current < 32768)
				result += Min((desired - current), Abs(deltaRate));
			else
				result -= Min((current + 65536 - desired), Abs(deltaRate));
		}
	}

	return (result & 65535);
	unguard;
}

void APawn::physicsRotation(FLOAT deltaTime, FVector OldVelocity)
{
	guard(APawn::physicsRotation);

	// Accumulate a desired new rotation.
	FRotator NewRotation = Rotation;	

	//FT
	if( !bRotateToDesired && bFixedRotationDir )
	{
		AActor::physicsRotation( deltaTime );
		return;
	}

//	if ( (!bRotateToDesired && !bFixedRotationDir)					// DPL: Undid FT's addition; was keeping flying pawns from banking
//		|| (bRotateToDesired && (Rotation == DesiredRotation)) )
//		return;

	//if (!IsA(APlayerPawn::StaticClass())) //don't pitch or yaw player
	{
		INT deltaYaw = appRound(RotationRate.Yaw * deltaTime);
		
		//bRotateToDesired = 1; //Pawns always have a "desired" rotation
		//bFixedRotationDir = 0;
	
		//YAW 
		if ( DesiredRotation.Yaw != NewRotation.Yaw )
			NewRotation.Yaw = fixedTurn(NewRotation.Yaw, DesiredRotation.Yaw, deltaYaw);

		//PITCH
		if ( DesiredRotation.Pitch != NewRotation.Pitch )
		{
				INT deltaPitch = appRound(RotationRate.Pitch * deltaTime);
				NewRotation.Pitch = fixedTurn(NewRotation.Pitch, DesiredRotation.Pitch, deltaPitch);

			/*
			//pawns pitch instantly
			NewRotation.Pitch = DesiredRotation.Pitch & 65535;
			//debugf("desired pitch %f actual pitch %f",DesiredRot.Pitch, NewRotation.Pitch);
			if ( NewRotation.Pitch < 32768 )
			{
				if (NewRotation.Pitch > RotationRate.Pitch) //bound pitch
					NewRotation.Pitch = RotationRate.Pitch;
			}
			else if (NewRotation.Pitch < 65536 - RotationRate.Pitch)
				NewRotation.Pitch = 65536 - RotationRate.Pitch;
		*/		
		  }

	}

	//ROLL
	if (RotationRate.Roll > 0) 
	{
		//pawns roll based on physics
		if ((Physics == PHYS_Walking) && Velocity.SizeSquared() < 40000.f)
		{
			FLOAT SmoothRoll = Min(1.f, 8.f * deltaTime);
			if (NewRotation.Roll < 32768)
				NewRotation.Roll = appRound(NewRotation.Roll * (1 - SmoothRoll));
			else
				NewRotation.Roll = appRound(NewRotation.Roll + (65536 - NewRotation.Roll) * SmoothRoll);
		}
		else
		{
			FVector RealAcceleration = (Velocity - OldVelocity)/deltaTime;
			if (RealAcceleration.SizeSquared() > 10000.f) 
			{
				FLOAT MaxRoll = 28000.f;
				if ( Physics == PHYS_Walking )
					MaxRoll = 4096.f;
				NewRotation.Roll = 0;

				RealAcceleration = RealAcceleration.TransformVectorBy(GMath.UnitCoords/NewRotation); //y component will affect roll

				if (RealAcceleration.Y > 0) 
					NewRotation.Roll = Min(RotationRate.Roll, appRound(RealAcceleration.Y * MaxRoll/AccelRate)); 
				else
					NewRotation.Roll = ::Max(65536 - RotationRate.Roll, appRound(65536.f + RealAcceleration.Y * MaxRoll/AccelRate));

				//smoothly change rotation
				Rotation.Roll = Rotation.Roll & 65535;
				if (NewRotation.Roll > 32768)
				{
					if (Rotation.Roll < 32768)
						Rotation.Roll += 65536;
				}
				else if (Rotation.Roll > 32768)
					Rotation.Roll -= 65536;
	
				FLOAT SmoothRoll = Min(1.f, 5.f * deltaTime);
				NewRotation.Roll = appRound(NewRotation.Roll * SmoothRoll + Rotation.Roll * (1 - SmoothRoll));

				//if ((NewRotation.Roll > MaxRoll) && (NewRotation.Roll < (65536 - MaxRoll)))
				//	debugf("Illegal roll for %f", RealAcceleration.Y);
			}
			else
			{
				FLOAT SmoothRoll = Min(1.f, 8.f * deltaTime);
				if (NewRotation.Roll < 32768)
					NewRotation.Roll = appRound(NewRotation.Roll * (1 - SmoothRoll));
				else
					NewRotation.Roll = appRound(NewRotation.Roll + (65536 - NewRotation.Roll) * SmoothRoll);
			}
		}
	}
	else
		NewRotation.Roll = 0;

	// Set the new rotation.
	if( NewRotation != Rotation )
	{
		FCheckResult Hit(1.f);
		GetLevel()->MoveActor( this, FVector(0,0,0), NewRotation, Hit );
	}

	unguard;
}

void AActor::physicsRotation(FLOAT deltaTime)
{
	guard(AActor::physicsRotation);
	
	if ( (!bRotateToDesired && !bFixedRotationDir)
		|| (bRotateToDesired && (Rotation == DesiredRotation)) )
		return;

	// Accumulate a desired new rotation.
	FRotator NewRotation = Rotation;	
	FRotator deltaRotation = RotationRate * deltaTime;

	//YAW
	if ( (deltaRotation.Yaw != 0) && (!bRotateToDesired || (DesiredRotation.Yaw != NewRotation.Yaw)) )
		NewRotation.Yaw = fixedTurn(NewRotation.Yaw, DesiredRotation.Yaw, deltaRotation.Yaw);
	//PITCH
	if ( (deltaRotation.Pitch != 0) && (!bRotateToDesired || (DesiredRotation.Pitch != NewRotation.Pitch)) )
		NewRotation.Pitch = fixedTurn(NewRotation.Pitch, DesiredRotation.Pitch, deltaRotation.Pitch);
	//ROLL
	if ( (deltaRotation.Roll != 0) && (!bRotateToDesired || (DesiredRotation.Roll != NewRotation.Roll)) )
		NewRotation.Roll = fixedTurn(NewRotation.Roll, DesiredRotation.Roll, deltaRotation.Roll);	

	// Set the new rotation.
	if( NewRotation != Rotation )
	{
		FCheckResult Hit(1.f);
		GetLevel()->MoveActor( this, FVector(0,0,0), NewRotation, Hit );
	}

	if ( bRotateToDesired && (Rotation == DesiredRotation) && IsProbing(NAME_EndedRotation) )
		eventEndedRotation(); //tell thing rotation ended

	unguard;
}

/*
physWalking()

*/
#if defined(LEGEND) // added by Legend 1/31/1999
//-----------------------------------------------------------------------------
// climable and frictionless texture support routines 
//-----------------------------------------------------------------------------
//const float FRICTION_SLIPPERY  = 0.f;
const float FRICTION_CLIMBABLE = 10.f;

static UTexture* TraceTexture
(
	AActor*			Actor,
	FCheckResult&	Hit,
	FVector			TraceEnd,
	FVector			TraceStart = FVector(0,0,0),
	FVector			TraceExtent = FVector(0,0,0)
)
{
	UModel* Model = Actor->XLevel->Model;
	UTexture* Texture = NULL;

	// Trace the line.
	Actor->GetLevel()->SingleLineCheck( Hit, Actor, TraceEnd, TraceStart, TRACE_VisBlocking, TraceExtent );

	// attempt to locate the surface/texture associated with the BSP Node (Hit.Item)
	if( Hit.Actor != NULL && Hit.Actor->IsA( ALevelInfo::StaticClass() ) )
	{
		const FBspNode*	Node = &Model->Nodes( Hit.Item );
		if( Node != NULL )
		{
			const FBspSurf* Surf = &Model->Surfs( Node->iSurf );
			Texture = Surf->Texture;
		}
	}

	return Texture;
}

static UTexture* CheckClimbSurface( APawn* Pawn, FCheckResult& Hit )
{
    FVector StartTrace, EndTrace;
	FRotator Rot;
	UTexture* Texture;

	Rot = Pawn->Rotation;
	Rot.Pitch = 0;

	// trace forward from just above the player's feet
    StartTrace = Pawn->Location - FVector(0,0,0.7) * Pawn->CollisionHeight;
    EndTrace = StartTrace + 2*Pawn->CollisionRadius * Rot.Vector();
    Texture = TraceTexture( Pawn, Hit, EndTrace, StartTrace );

	if( Texture == NULL || Texture->Friction != FRICTION_CLIMBABLE )
	{
		// check to the player's left
		Rot.Yaw -= 16384;
		EndTrace = StartTrace + 2*Pawn->CollisionRadius * Rot.Vector();
	    Texture = TraceTexture( Pawn, Hit, EndTrace, StartTrace );
	}
	if( Texture == NULL || Texture->Friction != FRICTION_CLIMBABLE )
	{
		// check to the player's right
		Rot.Yaw += 32768;
		EndTrace = StartTrace + 2*Pawn->CollisionRadius * Rot.Vector();
	    Texture = TraceTexture( Pawn, Hit, EndTrace, StartTrace );
	}
	if( Texture == NULL || Texture->Friction != FRICTION_CLIMBABLE )
	{
		// check behind the player
		Rot.Yaw += 16384;
		EndTrace = StartTrace + 2*Pawn->CollisionRadius * Rot.Vector();
	    Texture = TraceTexture( Pawn, Hit, EndTrace, StartTrace );
	}

	return Texture;
}

static UTexture* CheckWalkSurface( APawn* Pawn, FCheckResult& Hit )
{
    FVector StartTrace, EndTrace;
    UTexture* Texture;

	// trace from player origin to radius*2 below the collision cylinder
    StartTrace = Pawn->Location;
    EndTrace = StartTrace - FVector(0,0,1) * ( Pawn->CollisionHeight + Pawn->CollisionRadius * 2 );
    Texture = TraceTexture( Pawn, Hit, EndTrace, StartTrace );

	return Texture;
}

static bool CheckSurfaces( APawn* Pawn, FLOAT deltaTime, INT Iterations )
{
	FCheckResult Hit(1.f);
	UTexture* Texture;
	
	if( ! Pawn->Level->bCheckWalkSurfaces )
		return false;

	Texture = CheckClimbSurface( Pawn, Hit );
	if( Texture != NULL && Texture->Friction == FRICTION_CLIMBABLE )
	{
#if 1 //Fix added by Legend on 4/12/2000
		// fix for stuck animations when jumping onto ladders
		if( Pawn->Physics == PHYS_Falling )
		{
			Pawn->eventLanded( Hit.Normal );
			Pawn->setPhysics( PHYS_Walking );
		}
#endif

		Pawn->eventWalkTexture( Texture, Hit.Location, Hit.Normal );

		if( !Pawn->Acceleration.IsZero() )
		{
			// bias facing view up (so that moving directly forward causes pawns to climb)
			FRotator Rot = Pawn->ViewRotation;
			Rot.Pitch += 4096;
			Pawn->Velocity.Z = Pawn->GroundSpeed * Rot.Vector().Z;

#if 1 //Fix added by Legend on 4/12/2000
			// when moving up, reduce horizontal velocity to improve stability
			if( Rot.Pitch > 0 )
			{
				Pawn->Velocity.X *= 0.8;
				Pawn->Velocity.Y *= 0.8;
			}
#endif
		}
		Pawn->physFlying( deltaTime, Iterations );
		return true;
	}

	Texture = CheckWalkSurface( Pawn, Hit );
	Pawn->eventWalkTexture( Texture, Hit.Location, Hit.Normal );

	if( Pawn->Physics == PHYS_Walking && Texture != NULL && Texture->Friction < 1.f )
	{
		// compute slip direction
#if 1 //Fix added by Legend on 4/12/2000
		FVector Slide = (deltaTime * Pawn->Region.Zone->ZoneGravity/(0.5 * ::Max(0.02f, 4.f * Texture->Friction))) * deltaTime;
#else
		FVector Slide = (deltaTime * Pawn->Region.Zone->ZoneGravity/(0.5 * ::Max(0.05f, 4.f * Texture->Friction))) * deltaTime;
#endif
		FVector Delta = Slide - Hit.Normal * (Slide | Hit.Normal);
		if( (Delta | Slide) >= 0 )
			Pawn->GetLevel()->MoveActor( Pawn, Delta, Pawn->Rotation, Hit);
		return false;
	}

	return false;
}
//-----------------------------------------------------------------------------
#endif

// Perform a trace, 
bool Predict( FCheckResult& Hit, APawn* Pawn, FVector Start, FVector Vel, const FVector& Extent, float Time )
{
	const FLOAT MaxIter = 0.2f;
	const INT MaxIters = 10;
	FLOAT T = 0;
	Hit.Normal = FVector(0.f);
	for( INT i=0; i<MaxIters && T < Time; i++ )
	{
		FLOAT Iter = Min(MaxIter, Time-T);

		// Conform gravity to last hit surface.
		FVector Grav = Pawn->Region.Zone->ZoneGravity;;
		Grav -= Hit.Normal * (Grav|Hit.Normal);

		FVector End = Start + Vel * Iter + Grav * (Iter*Iter*0.5f);
		Pawn->GetLevel()->SingleLineCheck( Hit, Pawn, End, Start, TRACE_AllBlocking, Extent );
		Iter *= Hit.Time;
		T += Iter;
		Vel += Grav * Iter;

		if( Hit.Time < 1.f )
		{
			End = Start * (1.f-Hit.Time) + End * Hit.Time;
			if( Hit.Normal.Z > 0.7f )
			{
				// Landed.
				Hit.Location = End;
				Hit.Time = T/Time;
				return false;
			}
			else
			{
				// Hit wall. Test for mount.
				if( Pawn->MaxMountHeight > 0.f && Hit.Normal.Z > -0.1 )
				{
					// Make sure pawn is facing in direction of wall.
					if( (Hit.Normal | Pawn->Rotation.Vector()) < 0.f )
					{
						FBspSurf* Surf = Hit.BspSurf( Extent.MaxVal() );
 						if( Surf && (Surf->PolyFlags & PF_SpecialPoly) )
						{
							// Shortcut: Assume any hit on a mounting surface will result in a mount.
							Hit.Location = End;
							Hit.Time = T/Time;
							return false;
						}
					}
				}

				// Conform velocity to hit.
				Vel -= Hit.Normal * (Vel|Hit.Normal);
			}
		}
		else
			Hit.Normal = FVector(0.f);
		Start = End;
	}
	Hit.Location = Start;
	Hit.Time = 1.f;
	return true;
}

void APawn::physWalking(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physWalking);

#if defined(LEGEND) // added by Legend 1/31/1999
	if( CheckSurfaces( this, deltaTime, Iterations ) )
		return;
#endif

	if ( Region.ZoneNumber == 0 )
	{
		// not in valid spot
		if ( Role == ROLE_Authority )
			debugf( TEXT("%s fell out of the world!"), GetName() );
		eventFellOutOfWorld();
		return;
	}

	FVector GroundNormal(0,0,1);

	Acceleration.Z = 0;
	FVector AccelDir;
	if ( Acceleration.IsZero() )
		AccelDir = Acceleration;
	else
		AccelDir = Acceleration.SafeNormal();

	calcVelocity(AccelDir, deltaTime, GroundSpeed, Region.Zone->ZoneGroundFriction*(bNoZoneFriction?0:1), 0, 0, 0);   
	
	FVector DesiredMove = Velocity;
	if ( IsA(APlayerPawn::StaticClass()) || (Region.Zone->ZoneVelocity.SizeSquared() > 90000) )
	{
		// Add effect of velocity zone
		// Rather than constant velocity, hacked to make sure that velocity being clamped when walking doesn't 
		// cause the zone velocity to have too much of an effect at fast frame rates

		DesiredMove = DesiredMove + Region.Zone->ZoneVelocity * 25 * deltaTime;
	}

	//-------------------------------------------------------------------------------------------
	//Perform the move
	FVector GravDir = FVector(0,0,-1);
	if (Region.Zone->ZoneGravity.Z > 0)
		GravDir.Z = 1;
	FVector Down = GravDir * (MaxStepHeight + 2.f);
	FVector Extent = GetCylinderExtent();
	FCheckResult Hit(1.f);
	OldLocation = Location;
	bJustTeleported = 0;
	int bCheckedFall = 0;
	int bMustJump = 0;

	FLOAT remainingTime = deltaTime;
	FLOAT timeTick;
	while ( (remainingTime > 0.f) && (Iterations < 8) )
	{
		Iterations++;
		if ( (remainingTime > 0.05f) && (IsA(APlayerPawn::StaticClass()) ||
			(DesiredMove.SizeSquared() * remainingTime * remainingTime > 400.f)) )
				timeTick = Min(0.05f, remainingTime * 0.5f);
		else timeTick = remainingTime;
		remainingTime -= timeTick;
		FVector Delta = timeTick * DesiredMove;
		FVector subLoc = Location;
		FVector subMove = Delta;
		int bZeroMove = Delta.IsNearlyZero();
		if ( bZeroMove )
		{
			remainingTime = 0;
			bHitSlopedWall = 0;
		}
		else
		{
			FVector ForwardCheck = AccelDir * CollisionRadius;
			if ( !bAvoidLedges )
				ForwardCheck *= 0.5; 
			// if AI controlled, check for fall by doing trace forward
			// try to find reasonable walk along ledge
			if ( (!IsA(APlayerPawn::StaticClass()) || bIsWalking) && !bCanFly ) 
			{
				// check if clear in front
				FVector Destn = Location + Delta + ForwardCheck;
				GetLevel()->SingleLineCheck(Hit, this, Destn, Location, TRACE_VisBlocking);  
				if (Hit.Time == 1.f)
				{
					// clear in front - see if there is footing at walk destination
					FLOAT DesiredDist = Delta.Size();
					// check down enough to catch either step or slope
					FLOAT TestDown = ::Max( 4.f + MaxStepHeight + CollisionHeight, 4.f + CollisionHeight + CollisionRadius + DesiredDist);
					// try a point trace
					GetLevel()->SingleLineCheck(Hit, this, Destn + TestDown * GravDir, Destn, TRACE_VisBlocking);
					FLOAT MaxRadius = ::Min(14.f, 0.5f * CollisionRadius);
					// if point trace hit nothing, or hit a slope, do a trace with extent
					if ( (Hit.Time == 1.f)
						|| ((Hit.Normal.Z > 0.7) && (Hit.Time * TestDown > CollisionHeight + MaxStepHeight + 4.f) 
							&& (Hit.Time * TestDown > CollisionHeight + 4.f + appSqrt(1 - Hit.Normal.Z * Hit.Normal.Z) * (CollisionRadius + DesiredDist)/Hit.Normal.Z)) )
						GetLevel()->SingleLineCheck(Hit, this, Destn + GravDir * (MaxStepHeight + 4.f), Destn , TRACE_VisBlocking, FVector(MaxRadius, MaxRadius, CollisionHeight));
					if (Hit.Time == 1.f)
					{
						// We have a ledge!
						Destn = Location + DesiredDist * AccelDir + ForwardCheck;
						//first, try tracing back to get the ledge direction
						FVector DesiredDir = Delta/DesiredDist;
						FVector LedgeDown = GravDir * (CollisionHeight + 6.f);
						GetLevel()->SingleLineCheck(Hit, this, Location + LedgeDown - 2 * CollisionRadius * AccelDir, 
											Destn + LedgeDown, TRACE_VisBlocking);
						LedgeDown = GravDir * (MaxStepHeight + 6.f);
						FVector LedgeDir;
						int bMoveForward = 0;
						int bGoodMove = 0;
						if (Hit.Time < 1.f) //found a ledge
						{
							if ( bAvoidLedges )
							{
								LedgeDir = Hit.Normal;
								LedgeDir.Z = 0;
								Delta = -1 * DesiredSpeed * GroundSpeed * timeTick * LedgeDir;
								bMoveForward = 0;
								if ( bStopAtLedges )
									MoveTimer = -1;
								else
									MoveTimer -= 0.25f;
							}
							else
							{
								LedgeDir.X = Hit.Normal.Y;
								LedgeDir.Y = -1 * Hit.Normal.X;
								LedgeDir.Z = 0;
								LedgeDir = LedgeDir.SafeNormal();
								if ( (LedgeDir | AccelDir) < 0 )
									LedgeDir *= -1;
								FLOAT DP = (LedgeDir | AccelDir );
								bMoveForward = ( (DP < 0.5) || (bCanJump && (DP < 0.7)) );
								if ( DP < 0.7 )
									Delta = Min(0.8f, DesiredSpeed) * GroundSpeed * timeTick * LedgeDir;
								else
									Delta = DesiredSpeed * GroundSpeed * timeTick * LedgeDir;
							}
						}
						else 
						{
							Destn = Location + Delta + ForwardCheck;
							LedgeDir.X = DesiredDir.Y;
							LedgeDir.Y = -1 * DesiredDir.X;
							LedgeDir.Z = 0;
							bMoveForward = 1;
							Delta = Min(0.8f, DesiredSpeed) * GroundSpeed * timeTick * LedgeDir;
							Destn = Location + Delta;
							GetLevel()->SingleLineCheck(Hit, this, Destn, Location, TRACE_VisBlocking, Extent);
							if (Hit.Time == 1.f)
							{
								GetLevel()->SingleLineCheck(Hit, this, Destn + LedgeDown, Destn, TRACE_VisBlocking, Extent);
								if ( Hit.Time == 1.f ) //reflect delta about desiredir
									Delta *= -1;
								else 
									bGoodMove = 1;
							}
							else 
								bGoodMove = 1;
						}
						if ( IsA(APlayerPawn::StaticClass()) )
						{
							bMoveForward = 0;
							if ( !bGoodMove )
							{
								Destn = Location + Delta + ForwardCheck;
								GetLevel()->SingleLineCheck(Hit, this, Destn, Location, TRACE_VisBlocking, Extent);
								if ( Hit.Time == 1.f )
									GetLevel()->SingleLineCheck(Hit, this, Destn + LedgeDown, Destn, TRACE_VisBlocking, FVector(MaxRadius, MaxRadius, CollisionHeight));
								if (Hit.Time == 1.f)
								{
									Acceleration = FVector(0,0,0);
									Delta = FVector(0,0,0);
								}
							}
						}
						if ( bCanJump && bMoveForward )
						{
							if ( !IsProbing(NAME_MayFall) )
								Delta = AccelDir * DesiredDist;
							else if ( !bCheckedFall )
							{
								bCheckedFall = 1;
								bMoveForward = 0;
								eventMayFall();
								if ( bCanJump )
								{
									bMustJump = 1;
									Delta = AccelDir * DesiredDist;
								}
							}
						}
						if ( !bCanJump  ) //if can't jump, make sure this is valid
						{
							if ( bMoveForward ) //check if should just move forward
							{
								Destn = Location + DesiredDir * (DesiredDist + CollisionRadius);
								GetLevel()->SingleLineCheck(Hit, this, Destn, Location, TRACE_VisBlocking, Extent);
								if ( Hit.Time == 1.f )
								{
									GetLevel()->SingleLineCheck(Hit, this, Destn + LedgeDown, Destn, TRACE_VisBlocking, Extent);
									if ( Hit.Time < 1.f )
									{
										Destn = Location + DesiredDir * DesiredDist;
										GetLevel()->SingleLineCheck(Hit, this, Destn + LedgeDown, Destn, TRACE_VisBlocking, Extent);
									}
								}
								if ( Hit.Time < 1.f )
									Delta = DesiredDir * DesiredDist;
								else 
								{
									bMoveForward = 0;
									if ( appFrand() < 2 * timeTick )
										MoveTimer = -1.f;
									else
										MoveTimer -= 0.1f;
								}
							}
							if ( !bMoveForward && !bGoodMove )
							{
								Destn = Location + Delta + ForwardCheck;
								GetLevel()->SingleLineCheck(Hit, this, Destn, Location, TRACE_VisBlocking, Extent);
								if ( Hit.Time == 1.f )
									GetLevel()->SingleLineCheck(Hit, this, Destn + LedgeDown, Destn, TRACE_VisBlocking, Extent);
								else if ( (Hit.Normal | DesiredDir) < MinHitWall )
									MoveTimer = -1.f;
								if (Hit.Time == 1.f)
								{
									GetLevel()->SingleLineCheck
										(Hit, this, Location + LedgeDown, Location , TRACE_VisBlocking, FVector(MaxRadius, MaxRadius, CollisionHeight));
									remainingTime = 0.f;
									MoveTimer = -1.f;
									Acceleration = FVector(0,0,0);
									if ( Hit.Time == 1.f )
										Delta = -1 * GroundSpeed * timeTick * DesiredDir;
									else
										Delta = FVector(0,0,0);
								}
							}
						}
					}
				}
				subMove = Delta;
			}

			// check if might hit sloped wall, and decide if to change direction before move
			if ( bHitSlopedWall )
			{
				FLOAT DesiredDist = Delta.Size();
				FVector DesiredDir = Delta/DesiredDist;
				FVector CheckDir = DesiredDir * ::Max(30.f, DesiredDist + 4);
				GetLevel()->SingleLineCheck(Hit, this, Location + CheckDir, Location , TRACE_VisBlocking, Extent);
				bHitSlopedWall = ( (Hit.Time < 1.f) && (Hit.Normal.Z > 0.01f) && (Hit.Normal.Z < 0.7f) );
				if ( bHitSlopedWall )
				{
					Hit.Normal.Z = 0.f;
					Hit.Normal = Hit.Normal.SafeNormal();
					Delta = (Delta - Hit.Normal * (Delta | Hit.Normal));
				}
				else if ( IsA(APlayerPawn::StaticClass()) ) //make sure really done with sloped wall
				{
					FVector CheckLoc = Location;
					CheckLoc.Z = CheckLoc.Z - CollisionHeight + MaxStepHeight + 4;
					GetLevel()->SingleLineCheck(Hit, this, CheckLoc + 100 * DesiredDir, CheckLoc , TRACE_VisBlocking);
					bHitSlopedWall = ( (Hit.Time < 1.f) && (Hit.Normal.Z > 0.01f) && (Hit.Normal.Z < 0.7f) );
					if ( !bHitSlopedWall )
					{
						FVector LeftDir = FVector(DesiredDir.Y, -1 * DesiredDir.X, 0) + DesiredDir;
						LeftDir = LeftDir.SafeNormal();
						GetLevel()->SingleLineCheck(Hit, this, CheckLoc + 100 * LeftDir, CheckLoc , TRACE_VisBlocking);
						bHitSlopedWall = ( (Hit.Time < 1.f) && (Hit.Normal.Z > 0.01f) && (Hit.Normal.Z < 0.7f) );
					}
					if ( !bHitSlopedWall )
					{
						FVector LeftDir = FVector(-1 * DesiredDir.Y, DesiredDir.X, 0) + DesiredDir;
						LeftDir = LeftDir.SafeNormal();
						GetLevel()->SingleLineCheck(Hit, this, CheckLoc + 100 * LeftDir, CheckLoc , TRACE_VisBlocking);
						bHitSlopedWall = ( (Hit.Time < 1.f) && (Hit.Normal.Z > 0.01f) && (Hit.Normal.Z < 0.7f) );
					}
				} 
				GetLevel()->MoveActor(this, Delta, Rotation, Hit);
			}
			else
			{
				GetLevel()->MoveActor(this, Delta, Rotation, Hit);
				bHitSlopedWall = ( (Hit.Time < 1.f) && (Hit.Normal.Z > 0.01f) && (Hit.Normal.Z < 0.7f) );
			}

			if (Hit.Time < 1.f) //try to step up
			{
				if( Hit.Normal.Z > 0.7 )
					GroundNormal = Hit.Normal;
				FVector DesiredDir = Delta.SafeNormal();
				stepUp(GravDir, DesiredDir, Delta * (1.f - Hit.Time), Hit);
				if ( Physics == PHYS_Falling ) // pawn decided to jump up
				{
					FLOAT DesiredDist = subMove.Size();
					FLOAT ActualDist = (Location - subLoc).Size2D();
					remainingTime += timeTick * (1 - Min(1.f,ActualDist/DesiredDist)); 
					eventFalling();
					if ( Physics == PHYS_Falling ) 
					{
						if (remainingTime > 0.01f)
							physFalling(remainingTime, Iterations);
					}
					else if ( Physics == PHYS_Flying )
					{
						Velocity = FVector(0,0, AirSpeed);
						Acceleration = FVector(0,0,AccelRate);
						if (remainingTime > 0.01f)
							physFlying(remainingTime, Iterations);
					}
					return;
				}
				else if( Physics == PHYS_Projectile ) // pawn is mounting
					return;
			}

			if ( IsA(APawn::StaticClass()) && (Physics == PHYS_Swimming) ) //just entered water
			{
				((APawn *)this)->startSwimming(Velocity, timeTick, remainingTime, Iterations);
				return;
			}
		}

		APlayerPawn* PPawn = Cast<APlayerPawn>(this);
		if( PPawn && PPawn->bAutoJump )
		{
			static float MinAutoJumpCos = 0.25f;
			static float AutoJumpThreshold = 10.f;

			// Check whether center has crossed a ledge.
			FVector FootDel = FVector(0,0,-CollisionHeight);
			FVector SubFootDel = FVector(0,0,-CollisionHeight-4.f);
			if( !bZeroMove && GetLevel()->SingleLineCheck( Hit, this, Location + FootDel + Down, Location, TRACE_AllBlocking ) )
			{
				// Find exact location of ledge.
				FVector Back = Location - AccelDir * (CollisionRadius*1.5f);
				if( !GetLevel()->SingleLineCheck( Hit, this, Back + SubFootDel, Location + SubFootDel, TRACE_AllBlocking ) )
				{
					FLOAT Cos = Hit.Normal | AccelDir;
					if( Cos > MinAutoJumpCos )
					{
						// Crossed ledge this tick. See whether we'd really fall on ledge edge.
						// Move player out to ledge location.
						FLOAT t = Hit.Time;
						FVector EdgeLoc = Back * t + Location * (1.f-t);

						// Find where we fall, and where we jump.
						FVector Start = EdgeLoc + AccelDir * (CollisionRadius*1.5f/Cos);
						Predict( Hit, this, Start, Velocity, Extent, 2.f );
						FVector Fall = Hit.Location;

						Predict( Hit, this, EdgeLoc, Velocity + FVector(0,0,JumpZ), Extent, 2.f );
						FVector Jump = Hit.Location;

						if( Jump.Z - Fall.Z > AutoJumpThreshold )
						{
							// Jumping is better.
							// Move back to edge location, then jump.
							GetLevel()->MoveActor(this, EdgeLoc - Location, Rotation, Hit);
							PPawn->eventDoJump(1.f);
							remainingTime += timeTick * t;
							physFalling(remainingTime, Iterations);
							return;
						}
					}
				}
			}
		}
		
		//drop to floor
		if ( bZeroMove )
		{
			FVector Foot = Location - FVector(0,0,CollisionHeight);
			GetLevel()->SingleLineCheck( Hit, this, Foot - FVector(0,0,20), Foot, TRACE_VisBlocking );
			FLOAT FloorDist = Hit.Time * 20;
			bZeroMove = ((Base == Hit.Actor) && (FloorDist <= 4.6f) && (FloorDist >= 4.1f));
		}
		if ( !bZeroMove )
		{
			GetLevel()->SingleLineCheck( Hit, this, Location + Down, Location, TRACE_AllBlocking, Extent );
			FLOAT FloorDist = Hit.Time * (MaxStepHeight + 2.f);

			if( Hit.Time < 1.0f && Hit.Normal.Z > 0.7f )
				GroundNormal = Hit.Normal;
			if( Hit.Time< 1.f && (Hit.Actor!=Base || FloorDist>2.4f) )
			{
				GetLevel()->MoveActor(this, Down, Rotation, Hit);
				if (Hit.Actor != Base)
					SetBase(Hit.Actor);
				if ( IsA(APawn::StaticClass()) && (Physics == PHYS_Swimming) ) //just entered water
				{
					((APawn *)this)->startSwimming(Velocity, timeTick, remainingTime, Iterations);
					return;
				}
			}
			else if ( FloorDist < 1.9 )
			{
				FVector realNorm = Hit.Normal;
				GetLevel()->MoveActor(this, FVector(0,0,2.1f - FloorDist), Rotation, Hit);
				Hit.Time = 0;
				Hit.Normal = realNorm;
			}

			if( !bMustJump && Hit.Time<1.f && Hit.Normal.Z>=0.7f )  
			{
				if( (Hit.Normal.Z < 1.f) && ((Hit.Normal.Z * Region.Zone->ZoneGroundFriction) < 3.3f) ) //slide down slope, depending on friction and gravity
				{
					FVector Slide = (deltaTime * Region.Zone->ZoneGravity/(2 * ::Max(0.5f, Region.Zone->ZoneGroundFriction))) * deltaTime;
					Delta = Slide - Hit.Normal * (Slide | Hit.Normal);
					if( (Delta | Slide) >= 0 )
						GetLevel()->MoveActor(this, Delta, Rotation, Hit);
					if ( IsA(APawn::StaticClass()) && (Physics == PHYS_Swimming) ) //just entered water
					{
						((APawn *)this)->startSwimming(Velocity, timeTick, remainingTime, Iterations);
						return;
					}
				}				
			}
			else
			{
				if ( !bMustJump && bCanJump && !bCheckedFall && IsProbing(NAME_MayFall) )
				{
					bCheckedFall = 1;
					eventMayFall();
				}
				if ( !bMustJump && (!bCanJump || bIsWalking) ) 
				{
					Velocity = FVector(0,0,0);
					Acceleration = FVector(0,0,0);
					GetLevel()->FarMoveActor(this,OldLocation,0,0 );
					MoveTimer = -1.f;
					return;
				}
				else // falling
				{
					if( Hit.Time < 1.f )
						bHitSlopedWall = 1;
					FLOAT DesiredDist = subMove.Size();
					FLOAT ActualDist = (Location - subLoc).Size2D();
					if (DesiredDist == 0.f)
						remainingTime = 0;
					else
						remainingTime += timeTick * (1 - Min(1.f,ActualDist/DesiredDist)); 
					Velocity.Z = 0.f;
					eventFalling();
					if (Physics == PHYS_Walking)
						setPhysics(PHYS_Falling); //default if script didn't change physics
					if ( !bMustJump && (Physics == PHYS_Falling) )
					{
						FLOAT velZ = Velocity.Z;
						if (!bJustTeleported && (deltaTime > remainingTime))
							Velocity = (Location - OldLocation)/(deltaTime - remainingTime);
						Velocity.Z = velZ;
						if (remainingTime > 0.01f)
							physFalling(remainingTime, Iterations);
						return;
					}
					else 
					{
						Delta = remainingTime * DesiredMove;
						GetLevel()->MoveActor(this, Delta, Rotation, Hit); 
						remainingTime = 0;
					}
				}
			}
		}
	}

	//if ( Iterations > 7 )
	//	debugf("Over 7 iterations in physics!");
	// make velocity reflect actual move
	if (!bJustTeleported)
	{
		Velocity = (Location - OldLocation) / deltaTime;

		// Conform velocity to ground.
		FLOAT Vel = Velocity.Size();
		if( Vel > 0.f && GroundNormal.Z > 0.f )
		{
			Velocity.Z -= (GroundNormal | Velocity) / GroundNormal.Z;
			FLOAT Vel2 = Velocity.Size();
			if( Vel2 > 0.f )
				Velocity *= Vel / Vel2;
		}
	}
 	unguard;
}

/* calcVelocity()
Calculates new velocity and acceleration for pawn for this tick
bounds acceleration and velocity, adds effects of friction and momentum
// bBrake only for walking?
// fixme - what is right for air turn rate - make it a pawn var?
// e.g. Max(bFluid * airbraking, friction)
*/
void APawn::calcVelocity(FVector AccelDir, FLOAT deltaTime, FLOAT maxSpeed, FLOAT friction, INT bFluid, INT bBrake, INT bBuoyant)
{
	guard(APawn::calcVelocity);
	FLOAT effectiveFriction = ::Max((FLOAT)bFluid,friction); 
	INT bWalkingPlayer = ( IsA(APlayerPawn::StaticClass()) && bIsWalking );
	if (bBrake && Acceleration.IsZero()) 
	{
		FVector OldVel = Velocity;
		FVector SumVel = FVector(0,0,0);

		FLOAT RemainingTime = deltaTime;
		// subdivide braking to get reasonably consistent results at lower frame rates
		// (important for packet loss situations w/ networking)
		while( RemainingTime > 0.03f )
		{
			Velocity = Velocity - (2 * Velocity) * 0.03f * effectiveFriction; //don't drift to a stop, brake
			if ( (Velocity | OldVel) > 0.f )
				SumVel += 0.03f * Velocity/deltaTime;
			RemainingTime -= 0.03f;
		}
		Velocity = Velocity - (2 * Velocity) * RemainingTime * effectiveFriction; //don't drift to a stop, brake
		if ( (Velocity | OldVel) > 0.f )
			SumVel += RemainingTime * Velocity/deltaTime;
		Velocity = SumVel;
		if ( ((OldVel | Velocity) < 0.f)
			|| (Velocity.SizeSquared() < 100) )//brake to a stop, not backwards
			Velocity = FVector(0,0,0);
	}
	else
	{
		FLOAT VelSize = Velocity.Size();
		if ( bWalkingPlayer )
		{
			if (Acceleration.SizeSquared() > 0.09f * AccelRate * AccelRate)
					Acceleration = AccelDir * AccelRate * 0.3f;
		}
		else if (Acceleration.SizeSquared() > AccelRate * AccelRate)
			Acceleration = AccelDir * AccelRate;
		Velocity = Velocity - (Velocity - AccelDir * VelSize) * deltaTime * effectiveFriction;  
	}

	Velocity = Velocity * (1 - bFluid * friction * deltaTime) + Acceleration * deltaTime;

	if (!IsA(APlayerPawn::StaticClass()))
		maxSpeed *= DesiredSpeed;

	if ( bBuoyant )
		Velocity = Velocity + Region.Zone->ZoneGravity * deltaTime * (1.f - Buoyancy/Mass);

	if ( bWalkingPlayer && (Velocity.SizeSquared() > 0.09f * maxSpeed * maxSpeed) )
	{
		FLOAT speed = Velocity.Size();
		Velocity = Velocity/speed;
		Velocity *= ::Max(0.3f * maxSpeed, speed * (1 - deltaTime * 2 * effectiveFriction)); 
	}
	else if (Velocity.SizeSquared() > maxSpeed * maxSpeed)
	{
		Velocity = Velocity.SafeNormal();
		Velocity *= maxSpeed;
	}


	unguard;
}

bool APawn::Mount(const FVector& MountDir, FCheckResult &Hit)
{
	guard(APawn::Mount);
	if( MaxMountHeight > 0.f && Hit.Normal.Z > -0.1 && Hit.Normal.Z < 0.7 )
	{
		// Make sure pawn is facing in direction of wall.
		if( (Hit.Normal | Rotation.Vector()) < 0.f )
		{
			FVector Extent = GetCylinderExtent();
			FBspSurf* Surf = Hit.BspSurf( Extent.MaxVal() );
 			if( Surf && (Surf->PolyFlags & PF_SpecialPoly) )
			{
				// Determine mount destination.
				FVector MoveDir = -Hit.Normal;
				MoveDir.Z = 0;
				MoveDir.Normalize();

				// Check path for up, forward, down movement, accounting for sloped walls.
				FCheckResult Hit2;
				FVector Location1 = Location + MountDir * MaxMountHeight;
				FVector Location2 = Location1 + MoveDir * (2.0f * CollisionRadius + Hit.Normal.Z * MaxMountHeight);
				FVector Location3 = Location2 - MountDir * MaxMountHeight - MoveDir * (Hit.Normal.Z * MaxMountHeight);

				// Find resting height after moving up and forward.
				if( GetLevel()->SingleLineCheck( Hit2, this, Location3, Location2, TRACE_VisBlocking, Extent ) == 0 )
				{
					float Threshold = Physics == PHYS_Falling ? 0.f : MaxStepHeight;
					if( Hit2.Location.Z - Location.Z >= Threshold )
					{
						// See whether we can get there.
						FVector MountLoc = Hit2.Location;
						MountLoc.Z += 2.0f;			// Anticipate std walking physics offset.
						Location1.Z = MountLoc.Z;
						if( GetLevel()->SingleLineCheck( Hit2, this, Location1, Location, TRACE_VisBlocking, Extent ) 
						&& GetLevel()->SingleLineCheck( Hit2, this, MountLoc, Location1, TRACE_VisBlocking, Extent ) )
						{
							SetBase( Hit.Actor );
							eventMount( MountLoc - Location );
							return true;
						}
					}
				}
			}
		}
	}
	return false;
	unguard;
}

void APawn::stepUp(FVector GravDir, FVector DesiredDir, FVector Delta, FCheckResult &Hit)
{
	guard(APawn::stepUp);

	if( Mount( -GravDir, Hit ) )
		return;

	FVector Down = GravDir * MaxStepHeight;
	FVector Up = -1 * Down;
	GetLevel()->MoveActor(this, Up, Rotation, Hit); 
	GetLevel()->MoveActor(this, Delta, Rotation, Hit);
	if (Hit.Time < 1.f)
	{
		if ( IsA(APlayerPawn::StaticClass()) && Hit.Actor->IsA(ADecoration::StaticClass()) && ((ADecoration *)(Hit.Actor))->bPushable
			&& ((Hit.Normal | DesiredDir) < -0.9) )
		{
			bJustTeleported = true;
			Velocity *= Mass/(Mass + Hit.Actor->Mass);
			processHitWall(Hit.Normal, Hit.Actor);
			if ( Physics == PHYS_Falling )
				return;
		}
		else if ((Abs(Hit.Normal.Z) < 0.2f) && (Hit.Time * Delta.SizeSquared() > 144.f))
		{
			// If we travelled 12 units, try again.
			stepUp(GravDir, DesiredDir, Delta * (1 - Hit.Time), Hit);
			if ( Physics == PHYS_Falling )
				return;
		}
		else
		{
			// Slide along wall.
			processHitWall(Hit.Normal, Hit.Actor);
			FVector OriginalDelta = Delta;
			FVector OldHitNormal = Hit.Normal;
			Delta = (Delta - Hit.Normal * (Delta | Hit.Normal)) * (1.f - Hit.Time);
			if( (Delta | OriginalDelta) >= 0 )
			{
				GetLevel()->MoveActor(this, Delta, Rotation, Hit);
				if (Hit.Time < 1.f)
				{
					processHitWall(Hit.Normal, Hit.Actor);
					if ( Physics == PHYS_Falling )
						return;
					TwoWallAdjust(DesiredDir, Delta, Hit.Normal, OldHitNormal, Hit.Time);
					GetLevel()->MoveActor(this, Delta, Rotation, Hit);
				}
			}
		}
	}

	GetLevel()->MoveActor(this, Down, Rotation, Hit);

	if ((Hit.Time < 1.f) && (Hit.Normal.Z < 0.5f))
	{
		Delta = (Down - Hit.Normal * (Down | Hit.Normal))  * (1.f - Hit.Time);
		if( (Delta | Down) >= 0 )
			GetLevel()->MoveActor(this, Delta, Rotation, Hit);
	} 

	unguard;
}

void AActor::processHitWall(FVector HitNormal, AActor *HitActor)
{
	guard(AActor::processHitWall);

	if ( HitActor->IsA(APawn::StaticClass()) )
		return;
	APawn* ThisPawn = Cast<APawn>(this);
	if ( ThisPawn )
	{
		if ( Acceleration.IsZero() )
			return;
		FVector Dir = (ThisPawn->Destination - Location).SafeNormal();
		if ( Physics == PHYS_Walking )
		{
			HitNormal.Z = 0;
			Dir.Z = 0;
		}
		if ( ThisPawn->MinHitWall < (Dir | HitNormal) )
			return;
		if ( !IsProbing(NAME_HitWall) && (Physics != PHYS_Falling) )
		{
			//ThisPawn->MoveTimer = -1.f;  //FT: commented this out cause if a character is doing a moveto, and they barely
			                               //    touch a wall, the MoveTo will end right away.  This is not the way it should be.
			ThisPawn->bFromWall = 1;
			return;
		}
	}
	else if ( !IsProbing(NAME_HitWall) )
		return;
	eventHitWall(HitNormal, HitActor);
	unguard;
}

#pragma DISABLE_OPTIMIZATION 
void AActor::processLanded(FVector HitNormal, AActor *HitActor, FLOAT remainingTime, INT Iterations)
{
	guard(AActor::processLanded);

	if ( !bIsPawn && Region.Zone->bBounceVelocity && (Region.Zone->ZoneVelocity != FVector(0,0,0)) )
	{
		Velocity = Region.Zone->ZoneVelocity + FVector(0,0,80);
		return;
	}
	/*
	// This inconsistent bit of code just causes trouble -- jsp.
	if ( IsA(APawn::StaticClass()) ) //Check that it is a valid landing (not a BSP cut)
	{
		FCheckResult Hit(1.f);
		GetLevel()->SingleLineCheck(Hit, this, Location -  FVector(0,0,0.2f * CollisionHeight + 8),
			Location, TRACE_AllBlocking, 0.9f * GetCylinderExtent());  
		if ( Hit.Time == 1.f ) //Not a valid landing
		{
			FVector Adjusted = Location;
			if ( GetLevel()->FindSpot(1.1f * GetCylinderExtent(), Adjusted, 1, 0) && (Adjusted != Location) )
			{
				GetLevel()->FarMoveActor(this, Adjusted, 0, 0);
				Velocity.X += appFrand() * 60 - 30;
				Velocity.Y += appFrand() * 60 - 30; 
				return;
			}
		}
	}
	else */
	if ( IsA(ADecoration::StaticClass()) )
	{
		if ( ((ADecoration *)this)->numLandings < 5 ) // make sure its on a valid landing
		{
			FCheckResult Hit(1.f);
			GetLevel()->SingleLineCheck(Hit, this, Location -  FVector(0,0,(CollisionHeight + CollisionRadius + 8)),
				Location - FVector(0,0,(0.8f * CollisionHeight)) , TRACE_AllBlocking);  
			if ( !Hit.Actor )
			{
				FVector partExtent = 0.5 * GetCylinderExtent();
				partExtent.Z *= 2;
				int bQuad1 = GetLevel()->SingleLineCheck(Hit, this, Location + FVector(0.5f * CollisionRadius, 0.5f * CollisionRadius, -8),
					Location + FVector(0.5f * CollisionRadius, 0.5f * CollisionRadius, 0), TRACE_AllBlocking, partExtent);
				int bQuad2 = GetLevel()->SingleLineCheck(Hit, this, Location + FVector(-0.5f * CollisionRadius, 0.5f * CollisionRadius, -8),
					Location + FVector(-0.5f * CollisionRadius, 0.5f * CollisionRadius, 0), TRACE_AllBlocking, partExtent);
				int bQuad3 = GetLevel()->SingleLineCheck(Hit, this, Location + FVector(-0.5f * CollisionRadius, -0.5f * CollisionRadius, -8),
					Location + FVector(-0.5f * CollisionRadius, -0.5f * CollisionRadius, 0), TRACE_AllBlocking, partExtent);
				int bQuad4 = GetLevel()->SingleLineCheck(Hit, this, Location + FVector(0.5f * CollisionRadius, -0.5f * CollisionRadius, -8),
					Location + FVector(0.5f * CollisionRadius, -0.5f * CollisionRadius, 0), TRACE_AllBlocking, partExtent);
				
				if ( (bQuad1 + bQuad2 + bQuad3 + bQuad4 > 1) && !(bQuad1 + bQuad3 == 0) && !(bQuad2 + bQuad4 == 0) )
				{
					((ADecoration *)this)->numLandings++;
					Velocity = 2 * Clamp( -1.f * Velocity.Z, 30.f, 30.f + CollisionRadius) * 
								FVector((FLOAT)(bQuad1 + bQuad4 - bQuad2 - bQuad3), (FLOAT)(bQuad1 + bQuad2 - bQuad3 - bQuad4) , 0.5);
					return;
				}
			}
			if ( IsA(ACarcass::StaticClass()) && (HitNormal.Z < 0.9) && ((ACarcass *)this)->bSlidingCarcass )
			{
				if ( appFrand() < 0.2f )
					((ADecoration *)this)->numLandings++;
				Velocity = HitNormal * 120;
				Velocity.Z = 70;
				return;
			}	
			((ADecoration *)this)->numLandings = 0;
		}
		else
			((ADecoration *)this)->numLandings = 0;
	}

	eventLanded(HitNormal);
	if (Physics == PHYS_Falling)
	{
		if (IsA(APawn::StaticClass()))
			setPhysics(PHYS_Walking, HitActor);
		else
		{
			setPhysics(PHYS_None, HitActor);
			Velocity = FVector(0,0,0);
		}
	}
	if ((Physics == PHYS_Walking) && IsA(APawn::StaticClass()))
	{
		Acceleration = Acceleration.SafeNormal();
		if (remainingTime > 0.01f)
			((APawn *)this)->physWalking(remainingTime, Iterations);
	}

	unguard;
}
#pragma ENABLE_OPTIMIZATION 

void AActor::physFalling(FLOAT deltaTime, INT Iterations)
{
	guard(AActor::physFalling);

	//bound acceleration, falling object has minimal ability to impact acceleration
	APawn *ThisPawn = IsA(APawn::StaticClass()) ? (APawn*)this : NULL;

#if defined(LEGEND) // added by Legend 1/31/1999
	if( ThisPawn && CheckSurfaces( ThisPawn, deltaTime, Iterations ) )
		return;
#endif

	if ( Region.ZoneNumber == 0 )
	{
		// not in valid spot
		if ( (Role == ROLE_Authority)
			&& (IsA(AInventory::StaticClass()) || IsA(ADecoration::StaticClass()) || IsA(APawn::StaticClass())) )
			debugf( TEXT("%s fell out of the world!"), GetName() );
		eventFellOutOfWorld();
		return;
	}

	FLOAT BoundSpeed = 0; //Bound final 2d portion of velocity to this if non-zero
	FVector RealAcceleration = Acceleration;

	if (ThisPawn)
	{
		// For original Unreal air control, use ThisPawn->AirControl = 0.05f
		// test for slope to avoid using air control to climb walls
		FLOAT AirControl = ThisPawn->AirControl;
		if( AirControl > 0.15f )
		{
			FVector TestWalk = ( AirControl * ThisPawn->AccelRate * Acceleration.SafeNormal() + Velocity ) * deltaTime;
			TestWalk.Z = 0;
			FCheckResult Hit(1.f);
			GetLevel()->SingleLineCheck( Hit, this, Location + TestWalk, Location, TRACE_VisBlocking, FVector( CollisionRadius, CollisionRadius, CollisionHeight ) );
			if( Hit.Actor != NULL )
				AirControl = 0.05f;
		}

		// boost maxAccel to increase player's control when falling
		FLOAT maxAccel = ThisPawn->AccelRate * AirControl;
		FVector Velocity2D = Velocity;
		Velocity2D.Z = 0;
		Acceleration.Z = 0;
		FLOAT speed2d = Velocity2D.Size2D(); 
		if (speed2d < 10.f) //allow initial burst
			maxAccel = maxAccel + (10 - speed2d)/deltaTime;
		else if ( speed2d >= ThisPawn->GroundSpeed )
		{
			if ( AirControl <= 0.05f )
				maxAccel = 1.f;
			else 
				BoundSpeed = speed2d;
		}

		if (Acceleration.SizeSquared() > maxAccel * maxAccel)
		{
			Acceleration = Acceleration.SafeNormal();
			Acceleration = Acceleration * maxAccel;
		}
	}
	FLOAT remainingTime = deltaTime;
	FLOAT timeTick = 0.1f;
	int numBounces = 0;
	FCheckResult Hit(1.f);
	int AdjustApex = 0;

	while ( (remainingTime > 0.f) && (Iterations < 8) )
	{
		Iterations++;
		if (remainingTime > 0.1f)
			timeTick = Min(0.1f, remainingTime * 0.5f);
		else timeTick = remainingTime;

		remainingTime -= timeTick;
		OldLocation = Location;
		bJustTeleported = 0;

		FVector OldVelocity = Velocity;
		if (!Region.Zone->bWaterZone)
		{
			if ( IsA(ADecoration::StaticClass()) && ((ADecoration *)this)->bBobbing ) 
				Velocity = OldVelocity + 0.5 * (Acceleration + 0.5 * Region.Zone->ZoneGravity) * timeTick; //average velocity for tick
			else if ( IsA(APlayerPawn::StaticClass()) && ((APawn *)this)->FootRegion.Zone->bWaterZone && (OldVelocity.Z < 0) )
				Velocity = OldVelocity * (1 - ((APawn *)this)->FootRegion.Zone->ZoneFluidFriction * timeTick)
							+ 0.5 * (Acceleration + Region.Zone->ZoneGravity) * timeTick; 
			else
				Velocity = OldVelocity + 0.5 * (Acceleration + Region.Zone->ZoneGravity) * timeTick; //average velocity for tick
		}
		else
		{
			Velocity = OldVelocity * (1 - 2 * Region.Zone->ZoneFluidFriction * timeTick) 
					+ 0.5f * (Acceleration + Region.Zone->ZoneGravity * (1.f - Buoyancy/::Max(1.f,Mass))) * timeTick; 
		}

		if ( !AdjustApex && ((OldVelocity.Z > 0) != (Velocity.Z > 0))
			&& (Abs(OldVelocity.Z) > 5.f) && (Abs(Velocity.Z) > 5.f)) //sign of Z component changed
		{
			AdjustApex = 1;
			FLOAT part = Abs(OldVelocity.Z)/(Abs(OldVelocity.Z) + Abs(Velocity.Z));
			if ((part * timeTick > 0.015f) && ((1 - part) * timeTick > 0.015f))
			{
				remainingTime = remainingTime + timeTick * (1 - part);
				timeTick = timeTick * part;
				if (!Region.Zone->bWaterZone)
				{
					if ( IsA(ADecoration::StaticClass()) && ((ADecoration *)this)->bBobbing ) 
						Velocity = OldVelocity + 0.5 * (Acceleration + 0.5 * Region.Zone->ZoneGravity) * timeTick; //average velocity for tick
					else if ( IsA(APlayerPawn::StaticClass()) && ((APawn *)this)->FootRegion.Zone->bWaterZone  && (OldVelocity.Z < 0) )
						Velocity = OldVelocity * (1 - ((APawn *)this)->FootRegion.Zone->ZoneFluidFriction * timeTick)
									+ 0.5 * (Acceleration + Region.Zone->ZoneGravity) * timeTick; 
					else
						Velocity = OldVelocity + 0.5 * (Acceleration + Region.Zone->ZoneGravity) * timeTick; //average velocity for tick
				}
				else
					Velocity = OldVelocity * (1 - 2 * Region.Zone->ZoneFluidFriction * timeTick) 
					+ 0.5f * (Acceleration + Region.Zone->ZoneGravity * (1.f - Buoyancy/::Max(1.f,Mass))) * timeTick; 
			}
		}
		else
			AdjustApex = 0;
		if ( BoundSpeed != 0 )
		{
			// using air control, so make sure not exceeding acceptable speed
			FVector Vel2D = Velocity;
			Vel2D.Z = 0;
			if ( Vel2D.SizeSquared() > BoundSpeed * BoundSpeed )
			{
				Vel2D = Vel2D.SafeNormal();
				Vel2D = Vel2D * BoundSpeed;
				Vel2D.Z = Velocity.Z;
				Velocity = Vel2D;
			}
		}
		FVector ZoneVel = FVector(0,0,0);
		if ( !bIsPawn || IsA(APlayerPawn::StaticClass()) || (Region.Zone->ZoneVelocity.SizeSquared() > 40000) )
			ZoneVel = Region.Zone->ZoneVelocity;

		FVector Adjusted = (Velocity + ZoneVel) * timeTick;

		GetLevel()->MoveActor(this, Adjusted, Rotation, Hit);
		if ( bDeleteMe )
			return;
		else if ( ThisPawn && (Physics == PHYS_Swimming) ) //just entered water
		{
			remainingTime = remainingTime + timeTick * (1.f - Hit.Time);
			ThisPawn->startSwimming(OldVelocity, timeTick, remainingTime, Iterations);
			return;
		}
		else if ( Hit.Time < 1.f )
		{
			if ( Hit.Actor->IsA(APlayerPawn::StaticClass()) && IsA(ADecoration::StaticClass()) )
				((ADecoration *)this)->numLandings = ::Max(0, ((ADecoration *)this)->numLandings - 1); 
			if (bBounce && !Hit.Actor->IsA(AMover::StaticClass()) )
			{
				eventHitWall(Hit.Normal, Hit.Actor);
				if ( Physics == PHYS_None )
					return;
				else if ( numBounces < 2 )
					remainingTime += timeTick * (1.f - Hit.Time);
				numBounces++;
			}
			else
			{
				if (Hit.Normal.Z > 0.7)
				{
					remainingTime += timeTick * (1.f - Hit.Time);
					if (!bJustTeleported && (Hit.Time > 0.1f) && (Hit.Time * timeTick > 0.003f) )
						Velocity = (Location - OldLocation)/(timeTick * Hit.Time);
					processLanded(Hit.Normal, Hit.Actor, remainingTime, Iterations);
					return;
				}
				else
				{
					if( ThisPawn )
						ThisPawn->Mount(FVector(0,0,1), Hit);
					processHitWall(Hit.Normal, Hit.Actor);
					FVector OldHitNormal = Hit.Normal;
					FVector Delta = (Adjusted - Hit.Normal * (Adjusted | Hit.Normal)) * (1.f - Hit.Time);
					if( (Delta | Adjusted) >= 0 )
					{
						GetLevel()->MoveActor(this, Delta, Rotation, Hit);
						if (Hit.Time < 1.f) //hit second wall
						{
							if ( Hit.Normal.Z > 0.7f )
							{
								remainingTime = 0.f;
								processLanded(Hit.Normal, Hit.Actor, remainingTime, Iterations);
								return;
							}
							else 
								processHitWall(Hit.Normal, Hit.Actor);
		
							FVector DesiredDir = Adjusted.SafeNormal();
							TwoWallAdjust(DesiredDir, Delta, Hit.Normal, OldHitNormal, Hit.Time);
							int bDitch = ( (OldHitNormal.Z > 0) && (Hit.Normal.Z > 0) && (Delta.Z == 0) && ((Hit.Normal | OldHitNormal) < 0) );
							GetLevel()->MoveActor(this, Delta, Rotation, Hit);
							if ( bDitch || (Hit.Normal.Z > 0.7) )
							{
								remainingTime = 0.f;
								processLanded(Hit.Normal, Hit.Actor, remainingTime, Iterations);
								return;
							}
						}
					}
					FLOAT OldZ = OldVelocity.Z;
					OldVelocity = (Location - OldLocation)/timeTick;
					OldVelocity.Z = OldZ;
				}
			}
		}

		//if ( Iterations > 7 )
		//	debugf("More than 7 iterations in falling");
		if (!bBounce && !bJustTeleported)
		{
			// refine the velocity by figuring out the average actual velocity over the tick, and then the final velocity.
			// This particularly corrects for situations where level geometry affected the fall.
			Velocity = (Location - OldLocation)/timeTick - ZoneVel; //actual average velocity
			if ( (Velocity.Z < OldVelocity.Z) || (OldVelocity.Z >= 0) )
				Velocity = 2 * Velocity - OldVelocity; //end velocity has 2* accel of avg
			if (Velocity.SizeSquared() > Region.Zone->ZoneTerminalVelocity * Region.Zone->ZoneTerminalVelocity)
			{
				Velocity = Velocity.SafeNormal();
				Velocity *= Region.Zone->ZoneTerminalVelocity;
			}
		}
	}

	Acceleration = RealAcceleration;
	unguard;
}

void APawn::startSwimming(FVector OldVelocity, FLOAT timeTick, FLOAT remainingTime, INT Iterations)
{
	guard(APawn::startSwimming);
	//debugf("fell into water");
	FVector End = Location;
	findWaterLine(OldLocation, End);
	FLOAT waterTime = 0.f;
	if (End != Location)
	{	
		waterTime = timeTick * (End - Location).Size()/(Location - OldLocation).Size();
		remainingTime += waterTime;
		FCheckResult Hit(1.f);
		GetLevel()->MoveActor(this, End - Location, Rotation, Hit);
	}
	if (!bBounce && !bJustTeleported)
		{
			Velocity = (Location - OldLocation)/(timeTick - waterTime); //actual average velocity
			Velocity = 2 * Velocity - OldVelocity; //end velocity has 2* accel of avg
			if (Velocity.SizeSquared() > 16000000.f)
			{
				Velocity = Velocity.SafeNormal();
				Velocity *= 4000.f;
			}
		//FIXME - calc. velocity more correctly everywhere
		}
	if ((Velocity.Z > -160.f) && (Velocity.Z < 0)) //allow for falling out of water
		Velocity.Z = -80.f - Velocity.Size2D() * 0.7f; //smooth bobbing
	if (remainingTime > 0.01f)
		physSwimming(remainingTime, Iterations);

	unguard;
}

void APawn::physFlying(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physFlying);

	FVector AccelDir;

	if ( bCollideWorld && (Region.ZoneNumber == 0) )
	{
		// not in valid spot
		if ( !bIsPlayer )
		{
			debugf( TEXT("%s flew out of the world!"), GetName());
			GetLevel()->DestroyActor( this );
		}
		return;
	}
	if ( Acceleration.IsZero() )
		AccelDir = Acceleration;
	else
		AccelDir = Acceleration.SafeNormal();
	calcVelocity(AccelDir, deltaTime, AirSpeed, Region.Zone->ZoneFluidFriction, 1, 0, 0);  

	Iterations++;
	OldLocation = Location;
	bJustTeleported = 0;
	FVector ZoneVel;
	if ( IsA(APlayerPawn::StaticClass()) || (Region.Zone->ZoneVelocity.SizeSquared() > 90000) )
		ZoneVel = Region.Zone->ZoneVelocity;
	else
		ZoneVel = FVector(0,0,0);
	FVector Adjusted = (Velocity + ZoneVel) * deltaTime; 
	FCheckResult Hit(1.f);
	GetLevel()->MoveActor(this, Adjusted, Rotation, Hit);
	if (Hit.Time < 1.f) 
	{
		FVector GravDir = FVector(0,0,-1);
		if (Region.Zone->ZoneGravity.Z > 0)
			GravDir.Z = 1;
		FVector DesiredDir = Adjusted.SafeNormal();
		FVector VelDir = Velocity.SafeNormal();
		FLOAT UpDown = GravDir | VelDir;
		if ( (Abs(Hit.Normal.Z) < 0.2f) && (UpDown < 0.5f) && (UpDown > -0.2f) )
		{
			FLOAT stepZ = Location.Z;
			stepUp(GravDir, DesiredDir, Adjusted * (1.f - Hit.Time), Hit);
			OldLocation.Z = Location.Z + (OldLocation.Z - stepZ);
		}
		else
		{
			processHitWall(Hit.Normal, Hit.Actor);
			//adjust and try again
			FVector OldHitNormal = Hit.Normal;
			FVector Delta = (Adjusted - Hit.Normal * (Adjusted | Hit.Normal)) * (1.f - Hit.Time);
			if( (Delta | Adjusted) >= 0 )
			{
				GetLevel()->MoveActor(this, Delta, Rotation, Hit);
				if (Hit.Time < 1.f) //hit second wall
				{
					processHitWall(Hit.Normal, Hit.Actor);
					TwoWallAdjust(DesiredDir, Delta, Hit.Normal, OldHitNormal, Hit.Time);
					GetLevel()->MoveActor(this, Delta, Rotation, Hit);
				}
			}
		}
	}

	if (!bJustTeleported)
		Velocity = (Location - OldLocation) / deltaTime;

	unguard;
}

/* Swimming uses gravity - but scaled by (mass - buoyancy)/mass
This is used only by pawns 

*/
// findWaterLine is temporary until trace supports zone change notification
FLOAT APawn::Swim(FVector Delta, FCheckResult &Hit)
{
	guard(APawn::Swim);
	FVector Start = Location;
	FLOAT airTime = 0.f;
	GetLevel()->MoveActor(this, Delta, Rotation, Hit);
	FVector End = Location;
	if (!Region.Zone->bWaterZone) //then left water
	{
		findWaterLine(Start, End);
		if (End != Location)
		{
			airTime = (End - Location).Size()/Delta.Size();
			GetLevel()->MoveActor(this, End - Location, Rotation, Hit);
		}
	}
	return airTime;
	unguard;
}

//get as close to waterline as possible, staying on same side as currently
void APawn::findWaterLine(FVector Start, FVector &End)
{
	guard(APawn::findWaterLine);
	if ((End - Start).SizeSquared() < 1.f)
		return; //current value of End is acceptable

	FVector MidPoint = 0.5 * (Start + End);
	FPointRegion NewRegion = GetLevel()->Model->PointRegion( Level, MidPoint );
	if( NewRegion.Zone->bWaterZone != Region.Zone->bWaterZone )
		Start = MidPoint; 
	else
		End = MidPoint;

	findWaterLine(Start, End);

	unguard;
}

void APawn::physSwimming(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physSwimming);

#if defined(LEGEND) // added by Legend 1/31/1999
	if( CheckSurfaces( this, deltaTime, Iterations ) )
		return;
#endif

	if (!HeadRegion.Zone->bWaterZone && (Velocity.Z > 100.f))
		//damp positive Z out of water
		Velocity.Z = Velocity.Z * (1 - deltaTime);

	Iterations++;
	OldLocation = Location;
	bJustTeleported = 0;
	FVector AccelDir;
	if ( Acceleration.IsZero() )
		AccelDir = Acceleration;
	else
		AccelDir = Acceleration.SafeNormal();
	calcVelocity(AccelDir, deltaTime, WaterSpeed, Region.Zone->ZoneFluidFriction, 1, 0, 1);  
	FLOAT velZ = Velocity.Z;
	FVector ZoneVel;
	if ( IsA(APlayerPawn::StaticClass()) || (Region.Zone->ZoneVelocity.SizeSquared() > 90000) )
	{
		// Add effect of velocity zone
		// Rather than constant velocity, hacked to make sure that velocity being clamped when swimming doesn't 
		// cause the zone velocity to have too much of an effect at fast frame rates

		ZoneVel = Region.Zone->ZoneVelocity * 25 * deltaTime;
	}
	else
		ZoneVel = FVector(0,0,0);
	FVector Adjusted = (Velocity + ZoneVel) * deltaTime; 
	FCheckResult Hit(1.f);
	FLOAT remainingTime = deltaTime * Swim(Adjusted, Hit);

	if (Hit.Time < 1.f)
	{
		FVector GravDir = FVector(0,0,-1);
		if (Region.Zone->ZoneGravity.Z > 0)
			GravDir.Z = 1;
		FVector DesiredDir = Adjusted.SafeNormal();
		FVector VelDir = Velocity.SafeNormal();
		FLOAT UpDown = GravDir | VelDir;
		if ( (Abs(Hit.Normal.Z) < 0.2f) && (UpDown < 0.5f) && (UpDown > -0.2f) )
		{
			FLOAT stepZ = Location.Z;
			stepUp(GravDir, DesiredDir, Adjusted * (1.f - Hit.Time), Hit);
			OldLocation.Z = Location.Z + (OldLocation.Z - stepZ);
		}
		else
		{
			processHitWall(Hit.Normal, Hit.Actor);
			//adjust and try again
			FVector OldHitNormal = Hit.Normal;
			FVector Delta = (Adjusted - Hit.Normal * (Adjusted | Hit.Normal)) * (1.f - Hit.Time);
			if( (Delta | Adjusted) >= 0 )
			{
				remainingTime = remainingTime * (1.f - Hit.Time) * Swim(Delta, Hit);
				if(Hit.Time < 1.f) //hit second wall
				{
					processHitWall(Hit.Normal, Hit.Actor);
					TwoWallAdjust(DesiredDir, Delta, Hit.Normal, OldHitNormal, Hit.Time);
					remainingTime = remainingTime * (1.f - Hit.Time) * Swim(Delta, Hit);
				}
			}
		}
	}

	if (!bJustTeleported && (remainingTime < deltaTime))
	{
		int bWaterJump = (velZ != Velocity.Z); //changed by script
		if (bWaterJump)
			velZ = Velocity.Z;
		Velocity = (Location - OldLocation) / (deltaTime - remainingTime);
		if (bWaterJump)
			Velocity.Z = velZ;
	}

	if (!Region.Zone->bWaterZone)
	{
		if (Physics == PHYS_Swimming)
			setPhysics(PHYS_Falling); //in case script didn't change it (w/ zone change)
		if ((Velocity.Z < 160.f) && (Velocity.Z > 0)) //allow for falling out of water
			Velocity.Z = 40.f + Velocity.Size2D() * 0.4f; //smooth bobbing
	}

	if (remainingTime > 0.01f) //may have left water - if so, script might have set new physics mode
	{
		if (Physics == PHYS_Falling) 
			physFalling(remainingTime, Iterations);
		else if (Physics == PHYS_Flying)
			physFlying(remainingTime, Iterations);
	}

	unguard;
}

/* PhysProjectile is tailored for projectiles 
*/
void AActor::physProjectile(FLOAT deltaTime, INT Iterations)
{
	guard(AActor::physProjectile);

	//bound acceleration, calculate velocity, add effects of friction and momentum
	//friction affects projectiles less (more aerodynamic)
	FLOAT remainingTime = deltaTime;
	int numBounces = 0;

	if ( Region.ZoneNumber == 0 )
	{
		GetLevel()->DestroyActor( this );
		return;
	}

	OldLocation = Location;
	bJustTeleported = 0;
	FCheckResult Hit(1.f);

	while ( (remainingTime > 0.f) && (Iterations < 8) )
	{
		Iterations++;
		if ( Region.Zone->bWaterZone )
			Velocity = (Velocity * (1 - 0.2f * Region.Zone->ZoneFluidFriction * remainingTime));
		Velocity = Velocity	+ Acceleration * remainingTime;
		FLOAT timeTick = remainingTime;
		remainingTime = 0.f;

		if ( IsA(AProjectile::StaticClass()) 
			&& (Velocity.SizeSquared() > ((AProjectile *)this)->MaxSpeed * ((AProjectile *)this)->MaxSpeed) )
		{
			Velocity = Velocity.SafeNormal();
			Velocity *= ((AProjectile *)this)->MaxSpeed;
		}

		FVector Adjusted = Velocity * deltaTime; 
		Hit.Time = 1.f;
		GetLevel()->MoveActor(this, Adjusted, Rotation, Hit);
		
		if( Hit.Time<1.f && !bDeleteMe && !bJustTeleported )
		{
			FVector DesiredDir = Adjusted.SafeNormal();
			eventHitWall(Hit.Normal, Hit.Actor);
			if (bBounce && !Hit.Actor->IsA(AMover::StaticClass()) )
			{
				if (numBounces < 2)
					remainingTime = timeTick * (1.f - Hit.Time);
				numBounces++;
				if (Physics == PHYS_Falling)
					physFalling(remainingTime, Iterations);
			}
		}
	}

	//if ( Iterations > 7 )
	//	debugf("Projectile with too many physics iterations!");
	if (!bBounce && !bJustTeleported)
		Velocity = (Location - OldLocation) / deltaTime;

	unguard;
}

/*
physRolling() - intended for non-pawns which are rolling or sliding along a floor

*/

void AActor::physRolling(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physRolling);
	//bound acceleration
	//goal - support +-Z gravity, but not other vectors
	//note that Z components of velocity and acceleration are not zeroed
	FVector VelDir = Velocity.SafeNormal();
	FVector AccelDir = Acceleration.SafeNormal();
	Velocity = Velocity - (VelDir - AccelDir) * Velocity.Size()
		* deltaTime * Region.Zone->ZoneGroundFriction; 

	Velocity = Velocity * (1 - Region.Zone->ZoneFluidFriction * deltaTime) + Acceleration * deltaTime;
	FVector DesiredMove = Velocity + Region.Zone->ZoneVelocity;
	OldLocation = Location;
	bJustTeleported = 0;

	//-------------------------------------------------------------------------------------------
	//Perform the move
	FLOAT remainingTime = deltaTime;
	FLOAT timeTick = 0.1f;
	FVector GravDir = FVector(0,0,-1);
	if (Region.Zone->ZoneGravity.Z > 0)
		GravDir.Z = 1; 
	FVector Down = GravDir * 16.f;
	FCheckResult Hit(1.f);
	int numBounces = 0;
	while ( (remainingTime > 0.f) && (Iterations < 8) )
	{
		Iterations++;
		if (remainingTime > 0.1f)
			timeTick = Min(0.1f, remainingTime * 0.5f);
		else timeTick = remainingTime;

		remainingTime -= timeTick;
		FVector Delta = timeTick * DesiredMove;
		FVector SubMove = Delta;
		FVector SubLoc = Location;
		if (!Delta.IsNearlyZero())
		{
			GetLevel()->MoveActor(this, Delta, Rotation, Hit);
			if (Hit.Time < 1.f) 
			{
				eventHitWall(Hit.Normal, Hit.Actor);
				if (bBounce && !Hit.Actor->IsA(AMover::StaticClass()) )
				{
					if (numBounces < 2)
						remainingTime += timeTick * (1.f - Hit.Time);
					numBounces++;
				}
				else
				{
						//adjust and try again
						FVector OriginalDelta = Delta;
			
						// Try again.
						FVector OldHitNormal = Hit.Normal;
						Delta = (Delta - Hit.Normal * (Delta | Hit.Normal)) * (1.f - Hit.Time);
						if( (Delta | OriginalDelta) >= 0 )
						{
							GetLevel()->MoveActor(this, Delta, Rotation, Hit);
							if (Hit.Time < 1.f)
							{
								eventHitWall(Hit.Normal, Hit.Actor);
								FVector DesiredDir = DesiredMove.SafeNormal();
								TwoWallAdjust(DesiredDir, Delta, Hit.Normal, OldHitNormal, Hit.Time);
								GetLevel()->MoveActor(this, Delta, Rotation, Hit);
							}
						}
				}
			}
		}

		//drop to floor
		GetLevel()->MoveActor(this, Down, Rotation, Hit);
		FLOAT DropTime = Hit.Time;
		FLOAT DropHitZ = Hit.Normal.Z;
		if (DropTime < 1.f) //slide down slope, depending on friction and gravity 
		{
			if ((Hit.Normal.Z < 1.f) && ((Hit.Normal.Z * Region.Zone->ZoneGroundFriction) < 3.3f))
			{
				FVector Slide = (deltaTime * Region.Zone->ZoneGravity/(2 * ::Max(0.5f, Region.Zone->ZoneGroundFriction))) * deltaTime;
				Delta = Slide - Hit.Normal * (Slide | Hit.Normal);
				if( (Delta | Slide) >= 0 )
				{
					GetLevel()->MoveActor(this, Delta, Rotation, Hit);
					DropHitZ = ::Max(DropHitZ, Hit.Normal.Z);
				}
			}				
		}

		if ((DropTime == 1.f) || (DropHitZ < 0.7f)) //then falling
		{
			FVector AdjustUp = -1 * (Down * DropTime); 
			GetLevel()->MoveActor(this, AdjustUp, Rotation, Hit);
			FLOAT DesiredDist = SubMove.Size();
			FLOAT ActualDist = (Location - SubLoc).Size2D();
			remainingTime += timeTick * (1 - Min(1.f,ActualDist/DesiredDist)); 
			eventFalling();
			if (Physics == PHYS_Rolling)
				setPhysics(PHYS_Falling); //default if script didn't change physics
			if (Physics == PHYS_Falling)
			{
				if (!bJustTeleported && (deltaTime > remainingTime))
					Velocity = (Location - OldLocation)/(deltaTime - remainingTime);
				Velocity.Z = 0.f;

				if (remainingTime > 0.005f)
					physFalling(remainingTime, Iterations);
				return;
			}
			else 
			{
				Delta = remainingTime * DesiredMove;
				GetLevel()->MoveActor(this, Delta, Rotation, Hit); 
			}
		}
		else if( Hit.Actor != Base)
		{
			// Handle floor notifications (standing on other actors).
			//debugf("%s is now on floor %s",GetFullName(),Hit.Actor ? Hit.Actor->GetFullName() : "None");
			SetBase( Hit.Actor );
		}			//drop to floor

	}
	// make velocity reflect actual move
	if (!bJustTeleported)
		Velocity = (Location - OldLocation) / deltaTime;
 	unguard;
}


/*
physSpider()

*/
inline int APawn::checkFloor(FVector Dir, FCheckResult &Hit)
{
	GetLevel()->SingleLineCheck(Hit, 0, Location - MaxStepHeight * Dir, Location, TRACE_VisBlocking, GetCylinderExtent());
	if (Hit.Time < 1.f)
	{
		Floor = Hit.Normal;
		return 1;
	}
	return 0;
}

int APawn::findNewFloor(FVector OldLocation, FLOAT deltaTime, FLOAT remainingTime, int Iterations)
{
	guard(APawn::findNewFloor);

	//look for floor
	FCheckResult Hit(1.f);
	//debugf("Find new floor for %s", GetFullName());
	if ( checkFloor(FVector(0,0,1), Hit) )
		return 1;
	if ( checkFloor(FVector(0,1,0), Hit) )
		return 1;
	if ( checkFloor(FVector(0,-1,0), Hit) )
		return 1;
	if ( checkFloor(FVector(1,0,0), Hit) )
		return 1;
	if ( checkFloor(FVector(-1,0,0), Hit) )
		return 1;

	// Fall
	eventFalling();
	if (Physics == PHYS_Spider)
		setPhysics(PHYS_Falling); //default if script didn't change physics
	if (Physics == PHYS_Falling)
	{
		FLOAT velZ = Velocity.Z;
		if (!bJustTeleported && (deltaTime > remainingTime))
			Velocity = (Location - OldLocation)/(deltaTime - remainingTime);
		Velocity.Z = velZ;
		if (remainingTime > 0.005f)
			physFalling(remainingTime, Iterations);
	}

	return 0;

	unguard;
}

//#pragma DISABLE_OPTIMIZATION
void APawn::physSpider(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physSpider);

	//calculate velocity
	FVector AccelDir;
	if ( Acceleration.IsZero() ) 
	{
		AccelDir = Acceleration;
		FVector OldVel = Velocity;
		Velocity = Velocity - (2 * Velocity) * deltaTime * Region.Zone->ZoneGroundFriction; //don't drift to a stop, brake
		if ((OldVel | Velocity) < 0.f) //brake to a stop, not backwards
			Velocity = Acceleration;
	}
	else
	{
		AccelDir = Acceleration.SafeNormal();
		FLOAT VelSize = Velocity.Size();
		if (Acceleration.SizeSquared() > AccelRate * AccelRate)
			Acceleration = AccelDir * AccelRate;
		Velocity = Velocity - (Velocity - AccelDir * VelSize) * deltaTime * Region.Zone->ZoneGroundFriction;  
	}

	Velocity = Velocity + Acceleration * deltaTime;
	FLOAT maxSpeed = GroundSpeed * DesiredSpeed;
	Iterations++;

	if (Velocity.SizeSquared() > maxSpeed * maxSpeed)
	{
		Velocity = Velocity.SafeNormal();
		Velocity *= maxSpeed;
	}
	FVector ZoneVel;
	if ( Region.Zone->ZoneVelocity.SizeSquared() > 90000 )
		ZoneVel = Region.Zone->ZoneVelocity;
	else
		ZoneVel = FVector(0,0,0);
	FVector DesiredMove = Velocity + ZoneVel;
	FLOAT MoveSize = DesiredMove.Size();
	FVector DesiredDir = DesiredMove/MoveSize;

	//Perform the move
	// Look for supporting wall
	int bFindNewFloor = Floor.IsNearlyZero();
	FCheckResult Hit(1.f);
	FVector GravDir = -1 * Floor;
	FVector Down = GravDir * (MaxStepHeight + 4.f);
	DesiredRotation = Rotation;
	if (!bFindNewFloor)
	{
		GetLevel()->SingleLineCheck(Hit, 0, Location + Down, Location, TRACE_VisBlocking, GetCylinderExtent());
		bFindNewFloor = (Hit.Time == 1.f);
	}
	if (bFindNewFloor)
	{
		if ( !findNewFloor(Location, deltaTime, deltaTime, Iterations) ) //find new floor or fall
			return;
		else
		{
			GravDir = -1 * Floor;
			Down = GravDir * (MaxStepHeight + 4.f);
		}
	}

	DesiredRotation = Floor.Rotation();
	DesiredRotation.Pitch -= 16384;
	DesiredRotation.Roll = 0;

	// If the spider is walking on the floor set his rotation so he's facing correctly
	if ( Floor == FVector(0,0,1) )
	{
		DesiredRotation.Yaw = Rotation.Yaw;
	}


	// modify desired move based on floor
	FLOAT dotp = AccelDir | Floor;
	FVector realDir = DesiredDir;
	if ( (Floor.Z < 0.6) && (dotp > 0.9) )
	{
		Floor = FVector(0,0,0);
		eventFalling();
		setPhysics(PHYS_Falling); 
		physFalling(deltaTime, Iterations);
		return;
	}
	else
	{
		DesiredDir = DesiredDir - Floor * (DesiredDir | Floor);
		DesiredDir = DesiredDir.SafeNormal();
	}

	OldLocation = Location;
	bJustTeleported = 0;

	FLOAT remainingTime = deltaTime;
	FLOAT timeTick = 0.05f;
	DesiredMove = MoveSize * DesiredDir;
	while( remainingTime>0.f && Iterations<8 )
	{
		Iterations++;
		if (remainingTime > 0.05f)
			timeTick = Min(0.05f, remainingTime * 0.5f);
		else timeTick = remainingTime;
		remainingTime -= timeTick;
		FVector Delta = timeTick * DesiredMove;
		FVector subLoc = Location;
		FVector subMove = Delta;

		if (!Delta.IsNearlyZero())
		{
			GetLevel()->MoveActor(this, Delta, DesiredRotation, Hit);
			if( Hit.Time < 1.f )
			{
				if (Hit.Normal.Z >= 0)
				{
					if ( ((Hit.Normal | realDir) < 0) && ((Floor | realDir) < 0) ) 
						eventHitWall(Hit.Normal, Hit.Actor);
					else
					{
						FVector Combo = (Hit.Normal + Floor).SafeNormal();
						if ( (realDir | Combo) > 0.9 )
							eventHitWall(Hit.Normal, Hit.Actor);
					}
					Floor = Hit.Normal;
					GravDir = -1 * Floor;
					Down = GravDir * (MaxStepHeight + 4.f);
				}
				else if ( (Hit.Normal | realDir) < 0 ) 
					eventHitWall(Hit.Normal, Hit.Actor);
				else if ( (Floor | realDir) > 0.7 )
				{
					eventFalling();
					if (Physics == PHYS_Spider)
						setPhysics(PHYS_Falling); //default if script didn't change physics
					if (Physics == PHYS_Falling)
					{
						FLOAT velZ = Velocity.Z;
						if (!bJustTeleported && (deltaTime > remainingTime))
							Velocity = (Location - OldLocation)/(deltaTime - remainingTime);
						Velocity.Z = velZ;
						if (remainingTime > 0.005f)
							physFalling(remainingTime, Iterations);
						return;
					}
				}
				FVector DesiredDir = Delta.SafeNormal();
				stepUp(GravDir, DesiredDir, Delta * (1.f - Hit.Time), Hit);
				if (Physics == PHYS_Falling)
				{
					if (remainingTime > 0.005f)
						physFalling(remainingTime, Iterations);
					return;
				}
			}
		}

		//drop to floor
		GetLevel()->MoveActor(this, Down, Rotation, Hit);
		if (Hit.Time == 1.f) //then find new floor or fall
		{
			if ( findNewFloor(OldLocation, deltaTime, remainingTime, Iterations) )
			{
				GravDir = -1 * Floor;
				Down = GravDir * (MaxStepHeight + 4.f);
			}
			else
				return;
		}
		else 
		{
			Floor = Hit.Normal;
			if( Hit.Actor != Base && !Hit.Actor->IsA(APawn::StaticClass()) )
			// Handle floor notifications (standing on other actors).
				SetBase( Hit.Actor );
		}
	}

	// make velocity reflect actual move
	if (!bJustTeleported)
		Velocity = (Location - OldLocation) / deltaTime;
	unguard;
}
//#pragma ENABLE_OPTIMIZATION

void AActor::physTrailer(FLOAT deltaTime)
{
	guard(APawn::physTrailer);

	if ( !Owner )
		return;
	if ( DrawType == DT_Sprite )
	{
		if ( bTrailerPrePivot )
			GetLevel()->FarMoveActor(this, Owner->Location + PrePivot, 0, 1);
		else if (bTrailerSameRotation )
			GetLevel()->FarMoveActor(this, Owner->Location - Mass * Owner->Rotation.Vector(), 0, 1);
		else
			GetLevel()->FarMoveActor(this, Owner->Location, 0, 1);
		return;
	}

	FVector Loc = Owner->Location;
	FRotator trailRot;
	if( AnimBone != 0 && Owner->Mesh )
	{
		FCoords Coords = Owner->Mesh->GetBoneCoords( Owner, AnimBone-1 );
		Loc = Coords.Origin;
		trailRot = Coords.OrthoRotation();
	}
	else
	{
		if ( bTrailerPrePivot || bTrailerSameRotation )
			// Apply PrePivot in owner's frame.
			//Loc += PrePivot * Owner->Rotation;
			Loc += PrePivot.TransformVectorBy( GMath.UnitCoords * Owner->Rotation );
		if ( bTrailerSameRotation )
			trailRot = Owner->Rotation;
		else if ( Owner->Velocity.IsNearlyZero() )
			trailRot = FRotator(16384,0,0);
		else
			trailRot = (-1 * Owner->Velocity).Rotation();
	}

	
	//If we're not solid, just use MoveActor and give it the correct Delta
	if( !bBlockActors && !bCollideWorld )
	{
		// Also, not sure if this Delta vector in MoveActor will mess with Touch and untouch.  We'll find out...
		FCheckResult Hit(1.f);
		GetLevel()->MoveActor(this, Loc - Location, trailRot, Hit);
	}
	else
	{
		// FT added the .01 to Delta.  This makes it move a bit so you get collision notifications, although it may not work all the time.
		GetLevel()->FarMoveActor(this, Loc, 0, 1);
		FCheckResult Hit(1.f);
		GetLevel()->MoveActor(this, FVector(0.01,0,0), trailRot, Hit);
	}

	unguard;
}


FRotator AInterpolationPoint::GetDesiredRotationAtPosition(INT PosOffset, INT PauseNum, FVector NewLocation)
{
	guard(AInterpolationPoint::GetDesiredRotationAtPosition);

	if ( PosOffset < 0 )
	{
		// find final rotation for previous position
		if ( PauseNum > 0 )
			return GetDesiredRotationAtPosition(PosOffset+1,PauseNum-1,NewLocation);
		else if ( bInstantNextPath )
			return GetDesiredRotationAtPause(PauseNum,NewLocation);
		else if ( !Prev->bEndOfPath )
		{
			// find last pause of previous
			INT i=0;
			for (; Prev->Pause[i]>0; i++ );
			return Prev->GetDesiredRotationAtPosition(PosOffset+1,i,NewLocation);
		}
		else
			return GetDesiredRotationAtPause(PauseNum,NewLocation);
	}
	else if ( PosOffset == 0 )
		return GetDesiredRotationAtPause(PauseNum,NewLocation);
	else // PosOffset > 0
	{
		while ( (Pause[PauseNum+1] > 0.f) && (PosOffset > 0) )
		{
			PauseNum++;
			PosOffset--;
		}
		if ( bEndOfPath || (PosOffset == 0) || Next->bInstantNextPath )
			return GetDesiredRotationAtPause(PauseNum,NewLocation);
		else
			return Next->GetDesiredRotationAtPosition(PosOffset-1, 0,NewLocation);
	}
	unguard;
}

FRotator AInterpolationPoint::GetDesiredRotationAtPause(INT PauseNum, FVector NewLocation)
{
	guard(AInterpolationPoint::GetDesiredRotationAtPause);

	if ( ViewTargetTag[PauseNum] != NAME_None )
	{
		if ( !ViewTarget[PauseNum] || (ViewTarget[PauseNum]->Tag != ViewTargetTag[PauseNum]) )
		{
			// find viewtarget
			for (INT i=0; i<GetLevel()->Actors.Num(); i++)
				if ( GetLevel()->Actors(i) && (GetLevel()->Actors(i)->Tag == ViewTargetTag[PauseNum]) )
				{
					ViewTarget[PauseNum] = GetLevel()->Actors(i);
					break;
				}
		}
		if ( ViewTarget[PauseNum] )
			return (ViewTarget[PauseNum]->Location - NewLocation).Rotation();
		else
			return Rotation;
	}
	else
		return Rotation; 	
	unguard;
}

FCoords AInterpolationPoint::GetInterpolatedPosition(FCoords OldCoords, FVector StartControl, FLOAT PhysAlpha, INT PauseNum)
{
	guard(AInterpolationPoint::GetInterpolatedPosition);

	FCoords NewCoords;

	// if instant path, draw straight dotted line - this will only happen in the editor
	if ( bInstantNextPath )
	{
		NewCoords = GMath.UnitCoords / Rotation;
		NewCoords.Origin = Location;
		return NewCoords;
	}

	// Bezier spline interpolation.
	FLOAT R = 1.f - PhysAlpha;
	FLOAT W0 = R * R * R;
	FLOAT W1 = 3 * PhysAlpha * R * R;
	FLOAT W2 = 3 * PhysAlpha * PhysAlpha * R;
	FLOAT W3 = PhysAlpha * PhysAlpha * PhysAlpha;
	FVector NewLocation = W0*OldCoords.Origin + W1*(OldCoords.Origin + StartControl) + W2*(Location + EndControlPoint) + W3*Location;

	// calculate new desired rotation
	if ( bFaceMoveDirection )
	{
		FLOAT BackAlpha = PhysAlpha - 0.001f;
		if ( BackAlpha <= 0.f )
		{
			if ( Prev )
				NewCoords = GMath.UnitCoords / Prev->Rotation;
			else
				NewCoords = GMath.UnitCoords / Rotation;
		}
		else
		{
			FLOAT R = 1.f - BackAlpha;
			FLOAT W0 = R * R * R;
			FLOAT W1 = 3 * BackAlpha * R * R;
			FLOAT W2 = 3 * BackAlpha * BackAlpha * R;
			FLOAT W3 = BackAlpha * BackAlpha * BackAlpha;
			FVector PrevLoc = W0*OldCoords.Origin + W1*(OldCoords.Origin + StartControl) + W2*(Location + EndControlPoint) + W3*Location;
			NewCoords = GMath.UnitCoords / (NewLocation - PrevLoc).Rotation();
		}
	}
	//else
	//if( bDontAlterRotation )
	//{
	//	NewCoords = OldCoords;
	//	NewCoords.Origin = NewLocation;
	//}
	else if ( bNewRotationSmoothing )
	{
		// use Hermite interpolation for rotation
		FLOAT TwoA = PhysAlpha * PhysAlpha;
		FLOAT ThreeA = TwoA * PhysAlpha;
		FRotator A = GetDesiredRotationAtPosition(-2,PauseNum,NewLocation);
		FRotator B = GetDesiredRotationAtPosition(-1,PauseNum,NewLocation);
		FRotator C = GetDesiredRotationAtPosition(0,PauseNum,NewLocation);
		FRotator D = GetDesiredRotationAtPosition(1,PauseNum,NewLocation);
		FRotator NewRot = (2*ThreeA - 3*TwoA + 1) * B
						+ (-2*ThreeA + 3*TwoA) * C
						+ (ThreeA - 2*TwoA + PhysAlpha) * 0.5f * (C-A)
						+ (ThreeA - TwoA) * 0.5f * (D-B);

		NewCoords = GMath.UnitCoords / NewRot; 	
	}
	else if ( ViewTargetTag[PauseNum] != NAME_None )
	{
		if ( !ViewTarget[PauseNum] || (ViewTarget[PauseNum]->Tag != ViewTargetTag[PauseNum]) )
		{
			// find viewtarget
			for (INT i=0; i<GetLevel()->Actors.Num(); i++)
				if ( GetLevel()->Actors(i) && (GetLevel()->Actors(i)->Tag == ViewTargetTag[PauseNum]) )
				{
					ViewTarget[PauseNum] = GetLevel()->Actors(i);
					break;
				}
		}
		if ( ViewTarget[PauseNum] )
			NewCoords = GMath.UnitCoords / (ViewTarget[PauseNum]->Location - NewLocation).Rotation();
		else
			NewCoords = OldCoords;
	}
	else
	{
		FLOAT TurnAlpha = PhysAlpha;
		if ( bTurnChange )
			TurnAlpha = appSqrt(PhysAlpha);
		FCoords C = GMath.UnitCoords / Rotation; 	// FIXME - cache coords in interpolation point
		NewCoords.XAxis = R * OldCoords.XAxis + TurnAlpha * C.XAxis;
		NewCoords.XAxis.Normalize();
		NewCoords.ZAxis = R * OldCoords.ZAxis + TurnAlpha * C.ZAxis;
		NewCoords.YAxis = NewCoords.ZAxis ^ NewCoords.XAxis;
		NewCoords.YAxis.Normalize();
		NewCoords.ZAxis = NewCoords.XAxis ^ NewCoords.YAxis;
		NewCoords.ZAxis.Normalize();
	}

	NewCoords.Origin = NewLocation;
	return NewCoords;
	unguard;
}

//
// Interpolating along a path.
// InterpolationManager handles interpolation for its owner
//
void AInterpolationManager::performPhysics(FLOAT DeltaTime)
{
	guard(AInterpolationManager::performPhysics);

	if ( !Owner || Owner->bDeleteMe )
	{
		GetLevel()->DestroyActor(this);
		return;
	}
	FVector OldLocation = Owner->Location;
	FVector OldVelocity = Owner->Velocity;
	FLOAT RemainingTime = DeltaTime;
	FLOAT DesiredSpeed = 0.f;

	// Linear interpolate OldLoc to Dest->Location.
	while( PhysRate!=0.0f && Owner->bInterpolating && RemainingTime>0.0f )
	{
		FVector SegmentStart = Owner->Location;

		// Compute rate modifier.
		FLOAT RateModifier = 1.f;
		if( Dest )
		{
			if(Owner->IPSpeed)	 //check if actor is going to use its own speed 
			{
				DesiredSpeed = Owner->IPSpeed;
			}
			else	  
			if ( Dest->Next )
			{
				FLOAT CurrentSpeed = (Dest->DesiredSpeed > 0.f) ? Dest->DesiredSpeed : Owner->Velocity.Size();
				if ( Dest->Next->DesiredSpeed > 0.f )
					DesiredSpeed = CurrentSpeed * (1.f - PhysAlpha) + Dest->Next->DesiredSpeed * PhysAlpha;
				else
				{
					if ( Dest->DesiredSpeed > 0.f )
						DesiredSpeed = Dest->DesiredSpeed;
					else
						DesiredSpeed = 0.f;
				}					
			}

			// possibly modify rate to make time between interpolation points not constant, but based on length of path
			if ( DesiredSpeed > 0.f )
			{
				if ( Dest->PathDist > 0.f )
					RateModifier = DesiredSpeed/Dest->PathDist;		
			}
			else
			{
				// keep smooth transition across interpolationpoint boundaries
				// FIXME cache EndSize and StartSize
				FLOAT EndSize = Dest->EndControlPoint.Size();
				FLOAT StartSize = Dest->StartControlPoint.Size();
				RateModifier = 1 + PhysAlpha * (EndSize/StartSize - 1.f);
			}
		}
		// Update alpha.
		FLOAT OldAlpha  = PhysAlpha;
		FLOAT DestAlpha = PhysAlpha + PhysRate * RateModifier * RemainingTime;
		PhysAlpha       = Clamp( DestAlpha, 0.f, 1.f );
		INT bDone = 0;
		FLOAT ActualTime = RemainingTime;
		if ( (DestAlpha > 1.f) && (RemainingPause <= 0.f) )
			ActualTime = PhysRate * RateModifier * RemainingTime - DestAlpha + 1.f;
		
		FVector NewLocation = Owner->Location;
		FCoords DesiredCoords;
		// Move and rotate.
		if ( !Dest )
			bDone = 1;
		else 
		{
			if( RemainingPause > 0.f )
			{
				DesiredSpeed = 0.f;
				PhysAlpha = 0.f;
				DestAlpha = 0.f;
				RemainingPause -= RemainingTime;
				if ( RemainingPause < 0.f )
				{
					RemainingPause = 0.f;
					RemainingTime = -1 * RemainingPause;
					bDone = 1;
				}
				eventUpdateCamera((StartPause - RemainingPause)/StartPause);
				if ( Dest->ViewTargetTag[PauseNum] != NAME_None )
				{
					if ( !Dest->ViewTarget[PauseNum] || (Dest->ViewTarget[PauseNum]->Tag != Dest->ViewTargetTag[PauseNum]) )
					{
						// find viewtarget
						for (INT i=0; i<GetLevel()->Actors.Num(); i++)
							if ( GetLevel()->Actors(i) && (GetLevel()->Actors(i)->Tag == Dest->ViewTargetTag[PauseNum]) )
							{
								Dest->ViewTarget[PauseNum] = GetLevel()->Actors(i);
								break;
							}
					}
					if ( Dest->ViewTarget[PauseNum] )
						DesiredCoords = GMath.UnitCoords / (Dest->ViewTarget[PauseNum]->Location - Owner->Location).Rotation();
				}
				DesiredCoords.Origin = Owner->Location;
			}
			else
			{
				eventUpdateCamera(OldAlpha);
				FCoords OldCoords;
				FVector StartControlPoint;
				if( Dest->Prev )
				{
					//if( Dest->Prev->bDontAlterRotation )
					//	OldCoords = GMath.UnitCoords / Owner->Rotation;
					//else
						OldCoords = GMath.UnitCoords / Dest->Prev->Rotation;

					OldCoords.Origin = Dest->Prev->Location;
					StartControlPoint = Dest->Prev->StartControlPoint;
				}
				else
				{
					//if( Dest->bDontAlterRotation )
					//	OldCoords = GMath.UnitCoords / Owner->Rotation;
					//else
						OldCoords = GMath.UnitCoords / Dest->Rotation;

					OldCoords.Origin = Owner->Location;
					StartControlPoint = FVector(0.f,0.f,0.f);
				}

				DesiredCoords = Dest->GetInterpolatedPosition(OldCoords, StartControlPoint, PhysAlpha, PauseNum);	
				NewLocation = DesiredCoords.Origin;
				if ( Dest->bConstantSpeed && (DesiredSpeed > 0.f) && (PhysAlpha < 1.f) && !Dest->bInstantNextPath )
				{
					// possibly modify rate to achieve about constant velocity
					FLOAT NewSpeed = (NewLocation - SegmentStart).Size();
					NewSpeed /= ActualTime;

					if ( Abs(NewSpeed - DesiredSpeed) > 0.05 * DesiredSpeed )
					{
						DestAlpha = OldAlpha + PhysRate * RateModifier * ActualTime * DesiredSpeed/NewSpeed;
						PhysAlpha = Clamp( DestAlpha, 0.f, 1.f );
						DesiredCoords = Dest->GetInterpolatedPosition(OldCoords, StartControlPoint, PhysAlpha, PauseNum);	
						NewLocation = DesiredCoords.Origin;
						if ( (DestAlpha > 1.f) && (RemainingPause <= 0.f) )
							ActualTime = PhysRate * RateModifier * RemainingTime - DestAlpha + 1.f;
					}
				}

				if ( Dest->bFaceMoveDirection )
				{
					DesiredCoords = GMath.UnitCoords / (DesiredCoords.Origin - Owner->Location).Rotation();
					DesiredCoords.Origin = NewLocation;
				}
			}

			// Smooth Rotation
			if ( !Dest->bNewRotationSmoothing )
			{
				FCoords CurrentCoords = GMath.UnitCoords / Owner->Rotation;

				// find unsmoothed desired rate
				FVector DesiredRateX = (DesiredCoords.XAxis - OldDesiredX)/ActualTime;
				FVector DesiredRateZ = (DesiredCoords.ZAxis - OldDesiredZ)/ActualTime;
				OldDesiredX = DesiredCoords.XAxis;
				OldDesiredZ = DesiredCoords.ZAxis;

				FLOAT Smoothing = 1.f;
				if ( Dest->Prev )
				{
					if ( Dest->Prev->Smoothing == 0.f )
						Smoothing = 0.f;
					else
						Smoothing = 1.f/Dest->Prev->Smoothing;
				}
				// check if actual desired rate is in same direction as unsmoothed desired rate
				FVector SmoothedDesiredRateX = (DesiredCoords.XAxis - CurrentCoords.XAxis)/Smoothing;
				FVector SmoothedDesiredRateZ = (DesiredCoords.ZAxis - CurrentCoords.ZAxis)/Smoothing;

				if ( (SmoothedDesiredRateX | DesiredRateX) <= 0.f )
					DesiredRateX = SmoothedDesiredRateX;
				if ( (SmoothedDesiredRateZ | DesiredRateZ) <= 0.f )
					DesiredRateZ = SmoothedDesiredRateZ;

				// smooth between actual rate and desired rate
				if ( Smoothing != 0.f )
				{
					TurnRateX = Smoothing*ActualTime * DesiredRateX + (1.f - Smoothing*ActualTime) * TurnRateX;
					TurnRateZ = Smoothing*ActualTime * DesiredRateZ + (1.f - Smoothing*ActualTime) * TurnRateZ;

					DesiredCoords.XAxis = CurrentCoords.XAxis + ActualTime * TurnRateX;
					DesiredCoords.XAxis.Normalize();
					DesiredCoords.ZAxis = CurrentCoords.ZAxis + ActualTime * TurnRateZ;
					DesiredCoords.ZAxis.Normalize();
					DesiredCoords.YAxis = DesiredCoords.ZAxis ^ DesiredCoords.XAxis;
					DesiredCoords.YAxis.Normalize();
					DesiredCoords.ZAxis = DesiredCoords.XAxis ^ DesiredCoords.YAxis;
					DesiredCoords.ZAxis.Normalize();
				}
				TurnRateX = (DesiredCoords.XAxis - CurrentCoords.XAxis)/ActualTime;
				TurnRateZ = (DesiredCoords.ZAxis - CurrentCoords.ZAxis)/ActualTime;
			}
			DesiredCoords.Origin = FVector(0,0,0);
			FCheckResult Hit(1.f);

			if( Owner->bInterpolating_IgnoreRot )
				GetLevel()->MoveActor( Owner, NewLocation - Owner->Location, Owner->Rotation, Hit );
			else
				GetLevel()->MoveActor( Owner, NewLocation - Owner->Location, DesiredCoords.OrthoRotation(), Hit );

			if( !Owner )
				return;
		}
	/*	APawn* AmPawn = Cast<APawn>(Owner);
		if( AmPawn && AmPawn->Controller )
			AmPawn->Controller->Rotation = AmPawn->Rotation;
	  */
		// If overflowing, notify and go to next place.
		INT bForward = ( PhysRate>0.0f );

		if ( bForward && (DestAlpha>1.f) )
		{
			PhysAlpha = 0.0f;
			RemainingTime *= (DestAlpha - 1.0f) / (DestAlpha - OldAlpha);
			bDone = 1;
		}
		else if ( !bForward && (DestAlpha<0.f) )
		{
			PhysAlpha = 1.0f;
			RemainingTime *= (0.0f - DestAlpha) / (OldAlpha - DestAlpha);
			bDone = 1;
		}
		if ( bDone )
		{
			if( Dest )
				Dest->eventInterpolateEnd(this, bForward);
			else
				eventFinishedInterpolation(NULL);

            if (!Owner)
                return;

			if ( bInstantMove )
			{
				bInstantMove = 0;
				OldLocation = Owner->Location;
				SegmentStart = OldLocation;
			}
			// check if should quit early
			Owner->Velocity = (Owner->Location - SegmentStart)/DeltaTime;
			FLOAT NewSpeed = Owner->Velocity.Size();
			if ( NewSpeed >= DesiredSpeed )
				RemainingTime = 0.f;
		}
		else RemainingTime=0.0f;
	}
	Owner->Velocity = (Owner->Location - OldLocation)/DeltaTime;
 	FLOAT NewSpeed = Owner->Velocity.Size();
	if ( (DesiredSpeed > 0.f) && (NewSpeed > 1.05f * DesiredSpeed) )
	{
		// smooth back if too fast (can still happen on point transitions)
		FCheckResult Hit(1.f);
		GetLevel()->MoveActor( Owner, ((DesiredSpeed/NewSpeed) * (Owner->Location - OldLocation) + OldLocation - Owner->Location), Owner->Rotation, Hit );
		if( !Owner )
			return;
		Owner->Velocity = (Owner->Location - OldLocation)/DeltaTime;
	} 
	Owner->Acceleration = (Owner->Velocity - OldVelocity)/DeltaTime;

	unguard;
}

