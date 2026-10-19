/*=============================================================================
	UnPhysic.cpp: Actor physics implementation

	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Steven Polge 3/97
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"
#include "UnMesh.h"
#include "Hp2PhysicServices.h"

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

	hp2_rs_engine_physic_find_base( this, &GHp2PhysicServices );
	unguard;
}

void AActor::setPhysics(BYTE NewPhysics, AActor *NewFloor)
{
	guard(AActor::setPhysics);

	hp2_rs_engine_physic_set_physics( this, NewPhysics, NewFloor, &GHp2PhysicServices );
	unguard;
}

void AActor::performPhysics(FLOAT DeltaSeconds)
{
	guard(AActor::performPhysics);

	hp2_rs_engine_physic_perform_physics_actor( this, DeltaSeconds, &GHp2PhysicServices );
	unguard;
}


void APawn::performPhysics(FLOAT DeltaSeconds)
{
	guard(APawn::performPhysics);

	hp2_rs_engine_physic_perform_physics_pawn( this, DeltaSeconds, &GHp2PhysicServices );
	unguard;
}

int AActor::fixedTurn(int current, int desired, int deltaRate)
{
	guard(AActor::fixedTurn);

	return hp2_rs_engine_physic_fixed_turn( current, desired, deltaRate, bFixedRotationDir, bRotateToDesired );
	unguard;
}

void APawn::physicsRotation(FLOAT deltaTime, FVector OldVelocity)
{
	guard(APawn::physicsRotation);

	hp2_rs_engine_physic_physics_rotation_pawn( this, deltaTime, &OldVelocity, &GHp2PhysicServices );
	unguard;
}

void AActor::physicsRotation(FLOAT deltaTime)
{
	guard(AActor::physicsRotation);

	hp2_rs_engine_physic_physics_rotation_actor( this, deltaTime, &GHp2PhysicServices );
	unguard;
}

/*
physWalking()

*/
void APawn::physWalking(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physWalking);
	hp2_rs_engine_physic_phys_walking( this, deltaTime, Iterations, &GHp2PhysicServices );
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
	hp2_rs_engine_physic_calc_velocity( this, &AccelDir, deltaTime, maxSpeed, friction, bFluid, bBrake, bBuoyant, &GHp2PhysicServices );
	unguard;
}

bool APawn::Mount(const FVector& MountDir, FCheckResult &Hit)
{
	guard(APawn::Mount);
	return hp2_rs_engine_physic_mount( this, &MountDir, &Hit, &GHp2PhysicServices ) != 0;
	unguard;
}

void APawn::stepUp(FVector GravDir, FVector DesiredDir, FVector Delta, FCheckResult &Hit)
{
	guard(APawn::stepUp);
	hp2_rs_engine_physic_step_up( this, &GravDir, &DesiredDir, &Delta, &Hit, &GHp2PhysicServices );
	unguard;
}

void AActor::processHitWall(FVector HitNormal, AActor *HitActor)
{
	guard(AActor::processHitWall);
	hp2_rs_engine_physic_process_hit_wall( this, &HitNormal, HitActor, &GHp2PhysicServices );
	unguard;
}

#pragma DISABLE_OPTIMIZATION 
void AActor::processLanded(FVector HitNormal, AActor *HitActor, FLOAT remainingTime, INT Iterations)
{
	guard(AActor::processLanded);
	hp2_rs_engine_physic_process_landed( this, &HitNormal, HitActor, remainingTime, Iterations, &GHp2PhysicServices );
	unguard;
}
#pragma ENABLE_OPTIMIZATION 

void AActor::physFalling(FLOAT deltaTime, INT Iterations)
{
	guard(AActor::physFalling);
	hp2_rs_engine_physic_phys_falling( this, deltaTime, Iterations, &GHp2PhysicServices );
	unguard;
}

void APawn::startSwimming(FVector OldVelocity, FLOAT timeTick, FLOAT remainingTime, INT Iterations)
{
	guard(APawn::startSwimming);
	hp2_rs_engine_physic_start_swimming( this, &OldVelocity, timeTick, remainingTime, Iterations, &GHp2PhysicServices );
	unguard;
}

void APawn::physFlying(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physFlying);
	hp2_rs_engine_physic_phys_flying( this, deltaTime, Iterations, &GHp2PhysicServices );
	unguard;
}

/* Swimming uses gravity - but scaled by (mass - buoyancy)/mass
This is used only by pawns 

*/
// findWaterLine is temporary until trace supports zone change notification
FLOAT APawn::Swim(FVector Delta, FCheckResult &Hit)
{
	guard(APawn::Swim);
	return hp2_rs_engine_physic_swim( this, &Delta, &Hit, &GHp2PhysicServices );
	unguard;
}

//get as close to waterline as possible, staying on same side as currently
void APawn::findWaterLine(FVector Start, FVector &End)
{
	guard(APawn::findWaterLine);
	hp2_rs_engine_physic_find_water_line( this, &Start, &End, &GHp2PhysicServices );
	unguard;
}

void APawn::physSwimming(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physSwimming);
	hp2_rs_engine_physic_phys_swimming( this, deltaTime, Iterations, &GHp2PhysicServices );
	unguard;
}

/* PhysProjectile is tailored for projectiles 
*/
void AActor::physProjectile(FLOAT deltaTime, INT Iterations)
{
	guard(AActor::physProjectile);
	hp2_rs_engine_physic_phys_projectile( this, deltaTime, Iterations, &GHp2PhysicServices );
	unguard;
}

/*
physRolling() - intended for non-pawns which are rolling or sliding along a floor

*/

void AActor::physRolling(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physRolling);
	hp2_rs_engine_physic_phys_rolling( this, deltaTime, Iterations, &GHp2PhysicServices );
	unguard;
}


/*
physSpider()

*/
inline int APawn::checkFloor(FVector Dir, FCheckResult &Hit)
{
	guard(APawn::checkFloor);
	return hp2_rs_engine_physic_check_floor( this, &Dir, &Hit, &GHp2PhysicServices );
	unguard;
}

int APawn::findNewFloor(FVector OldLocation, FLOAT deltaTime, FLOAT remainingTime, int Iterations)
{
	guard(APawn::findNewFloor);
	return hp2_rs_engine_physic_find_new_floor( this, &OldLocation, deltaTime, remainingTime, Iterations, &GHp2PhysicServices );
	unguard;
}

//#pragma DISABLE_OPTIMIZATION
void APawn::physSpider(FLOAT deltaTime, INT Iterations)
{
	guard(APawn::physSpider);
	hp2_rs_engine_physic_phys_spider( this, deltaTime, Iterations, &GHp2PhysicServices );
	unguard;
}
//#pragma ENABLE_OPTIMIZATION

void AActor::physTrailer(FLOAT deltaTime)
{
	guard(AActor::physTrailer);
	hp2_rs_engine_physic_phys_trailer( this, deltaTime, &GHp2PhysicServices );
	unguard;
}


FRotator AInterpolationPoint::GetDesiredRotationAtPosition(INT PosOffset, INT PauseNum, FVector NewLocation)
{
	guard(AInterpolationPoint::GetDesiredRotationAtPosition);
	FRotator Result;
	hp2_rs_engine_physic_get_desired_rotation_at_position( this, PosOffset, PauseNum, &NewLocation, &Result, &GHp2PhysicServices );
	return Result;
	unguard;
}

FRotator AInterpolationPoint::GetDesiredRotationAtPause(INT PauseNum, FVector NewLocation)
{
	guard(AInterpolationPoint::GetDesiredRotationAtPause);
	FRotator Result;
	hp2_rs_engine_physic_get_desired_rotation_at_pause( this, PauseNum, &NewLocation, &Result, &GHp2PhysicServices );
	return Result;
	unguard;
}

FCoords AInterpolationPoint::GetInterpolatedPosition(FCoords OldCoords, FVector StartControl, FLOAT PhysAlpha, INT PauseNum)
{
	guard(AInterpolationPoint::GetInterpolatedPosition);
	FCoords Result;
	hp2_rs_engine_physic_get_interpolated_position( this, &OldCoords, &StartControl, PhysAlpha, PauseNum, &Result, &GHp2PhysicServices );
	return Result;
	unguard;
}

//
// Interpolating along a path.
// InterpolationManager handles interpolation for its owner
//
void AInterpolationManager::performPhysics(FLOAT DeltaTime)
{
	guard(AInterpolationManager::performPhysics);
	hp2_rs_engine_physic_interpolation_perform_physics( this, DeltaTime, &GHp2PhysicServices );
	unguard;
}

