//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999
//
//  File:       UnParticle.cpp
//
//  Contents:   Implementation of UnParticle.cpp
//
//  History:    23-June-99   PKeet  Created
//
//	To do:		
//				Add bLanded and disable anymore collision checks
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"
#include "UnParticle.h"

#include <math.h>

/*-----------------------------------------------------------------------------
	UParticle object implementation.
-----------------------------------------------------------------------------*/

UParticle::UParticle(const FVector& Pos, const FVector& Vel, const AParticleFX* System)
	:	LastPosition(Pos), Position(Pos), Velocity(Vel), Age(0.0f), Id(-1) 
{
}


bool UParticle::Update( const FVector& SystemGravity, const FLOAT SystemDamping, const FVector& SystemWind, ULevel* Level, FLOAT Timestep, AParticleFX* System)
{
	// check for over lifetime (death)
	if ( Lifetime > 0.0f ) 
	{
		// returning false notifies the particle system that this particle has expired
		if ( Age+Timestep >= Lifetime )
			return false;
	}

#if STATS
//	GStat.ParticlesAlive++;
#endif

	// special case, need to grow to fit designer spec's by the time we are ready to fall
	if ( DripTimer > 0.0f )
	{
		DripTimer -= Timestep; 
		
		if( DripTimer <= 0.0f ) 
		{
			Timestep = -DripTimer;
			DripTimer = 0.0f;
			WidthDelta = 0.0f;
			LengthDelta = 0.0f;
		}
	}

	// update last known position to current
	LastPosition = Position;

	// regular particle behavior.  
	if ( DripTimer == 0.0f ) 
	{
		Spin += SpinRate*Timestep;

		// Integration is exact, for constant gravity and damping.
		if ( SystemDamping < 1.0f )
		{
			//
			// Fade velocity toward terminal velocity.
			//
			//	V = Vf + (Vo-Vf) * e^(-d t)
			//	X = Xo + Vf * t + (Vf-Vo)/d * (e^(-d t) - 1)
			//

			// Terminal velocity = gravity / damping.
			// Add wind to terminal velocity.			
			FVector Vfw = SystemGravity / System->Damping;
			if ( System->bWindPerParticle )
				// Compute wind response per particle location.
				Vfw += System->WindModifier * AWind::GetTotalWind(Level, Position);
			else
				Vfw += SystemWind;

			Position += Vfw*Timestep + (Vfw-Velocity) * ((SystemDamping - 1.f)/System->Damping);
			Velocity = Vfw + (Velocity-Vfw)*SystemDamping;
		}
		else
		{
			// No wind response.
			// V = Vo + a * t
			// X = Xo + Vo * t + 1/2 a * t^2
			Position += (Velocity + SystemGravity*(Timestep*0.5)) * Timestep;
			Velocity += SystemGravity * Timestep;
		}

		// If particles locations are relative to the system
		if (System->bSystemRelative) 
		{
			Position -= (System->LastUpdateLocation - System->Location);
			Position = System->Location + ((Position - System->Location).TransformVectorBy(GMath.UnitCoords / System->LastUpdateRotation)).TransformVectorBy(GMath.UnitCoords * System->Rotation);
		}

		// If System has Attraction enabled
		if ( !System->Attraction.IsZero() )
		{
			FVector Direction( System->Location - Position );
			Velocity.X += System->Attraction.X * Timestep * Direction.X;
			Velocity.Y += System->Attraction.Y * Timestep * Direction.Y;
			Velocity.Z += System->Attraction.Z * Timestep * Direction.Z;
		}

		// If the particle has a ChaosDelay 
		if ( ChaosDelay )
		{
			// Decrement particles ChaosDelay 
			ChaosDelay = ChaosDelay > Timestep ? ChaosDelay - Timestep : 0.0f;
		}
		
		// If the System has Chaos enabled and the particle has waited long enough, perturb velocity
		if ( System->Chaos && ChaosDelay <= 0.0f )
		{
			// calculate random vector and normalize
			FVector ChaosAdjust( -1.0f + 2.0f * appFrand(), -1.0f + 2.0f * appFrand(), -1.0f + 2.0f * appFrand() );
			ChaosAdjust.Normalize();
 
			// Perturb Velocity by Chaos value in random direction
			// NOTE:  Keeping Timestep out of the calculation should minimize framerate dependent behavior
			Velocity += ChaosAdjust * System->Chaos;// * Timestep;
			// set particle's delay back to full value
			ChaosDelay = System->ChaosDelay;
		}
	}

	// regular systems have bUpdate set to true.  Scripted systems might not
	if ( System->bUpdate ) 
	{
		if ( Age < System->AlphaGrowPeriod * Lifetime )
		{
			// Still fading in.
			Alpha += AlphaFull * Timestep / (System->AlphaGrowPeriod * Lifetime);
			Alpha = Min(Alpha, AlphaFull);
		}
		else if ( Age > System->AlphaDelay )
			Alpha += AlphaDelta * Timestep;

		//fix was getting -0.0f and -0.1f even with Clamp(Alpha, 0.0f, 1.0f)
		if ( Alpha < 0.001f ) 
			Alpha = 0.0f;

		// if we are using a palette, figure out which index we are at using percentage of life  //fix looping ?
		if ( System->ColorPalette )
		{
			if ( System->ColorPalette->Palette ) 
			{
				if (( Lifetime > 0.0f ) && ( Age > System->ColorDelay))
					Color = System->ColorPalette->Palette->Colors( (INT)((Age / Lifetime)*255.0f) ).Plane();
				else
					Color = System->ColorPalette->Palette->Colors(0).Plane();
			}
		}
		// if we aren't using a palette and we have a colordelta, use it
		else if ((ColorDelta.Size() > 0.0f) && (Age > System->ColorDelay)) 
		{
			Color.X += (ColorDelta.X * Timestep);
			Color.Y += (ColorDelta.Y * Timestep);
			Color.Z += (ColorDelta.Z * Timestep);
		}


		if (( System->RenderPrimitive != PPRIM_Liquid ) || ( DripTimer > 0.0f ))
		{
			if ( Age < System->SizeGrowPeriod * Lifetime )
			{
				// Still growing.
				FLOAT Step = Timestep / (System->SizeGrowPeriod * Lifetime);
				Width = Min(Width + WidthFull * Step, WidthFull);
				Length = Min(Length + LengthFull * Step, LengthFull);
			}
			else if ( Age > System->SizeDelay )
			{
				Width += WidthDelta * Timestep;
				Length += LengthDelta * Timestep;
			}
		}
	}

	if ( System->Elasticity > 0.0f && !Level->Model->FastLineCheck(LastPosition, Position) )
	{
		//	bounces++;
		//	if (bounces > MaxBounces)

		//	B = B.SafeNormal();
		//	*(FVector*)Result = A - 2.f * B * (B | A);

		FCheckResult Hit(1.f);
		System->GetLevel()->SingleLineCheck( Hit, NULL, Position, LastPosition, TRACE_VisBlocking);

		if (Hit.Actor != NULL && Hit.Actor->IsA(ALevelInfo::StaticClass()))
		{
			Velocity = Velocity - 2.0f * Hit.Normal * ( Velocity | Hit.Normal );

			Velocity *= System->Elasticity;

			Position = Hit.Location;
		}
		else
		{
			Velocity = FVector(0,0,0);
			Position = LastPosition;
		}
	}

	// Age the particle
	Age += Timestep;

	return true;
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/
