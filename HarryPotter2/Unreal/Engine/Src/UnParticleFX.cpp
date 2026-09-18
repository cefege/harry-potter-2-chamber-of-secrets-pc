//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999.
//
//  File:       UnParticleFX.cpp
//
//  Contents:   Implementation of AParticleFX
//
//  History:    26-April-99   PKeet  Created
//
//	To do:		
//				Put checks on AlphaDelay, ColorDelay and SizeDelay
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"
#include "UnParticle.h"
#include "UnParticleList.h"
#include "UnMesh.h"
#include "UnRender.h"
#include "UnStat.h"

#include <list>
using namespace std;

/*-----------------------------------------------------------------------------
	Constants.
-----------------------------------------------------------------------------*/

//15 fps
const float MAX_TIMESLICE=0.06667f; 

/*-----------------------------------------------------------------------------
	AParticleFX implementation.
-----------------------------------------------------------------------------*/

inline FLOAT MaxVal( const FFloatParams& P )
{
	return P.Base + Max(P.Rand, 0.0f);
}

inline FLOAT MinVal( const FFloatParams& P )
{
	return P.Base + Min(P.Rand, 0.0f);
}

//----------------------------------------------------------------------------

AParticleFX::AParticleFX()
{
	ParticleList = NULL;
	
	CreateParticleList();

	Age	= 0.0f;
	ElapsedTime = 0.0f;
	bUpdate = true;
	LOD = 1.0f;
}

//----------------------------------------------------------------------------

void AParticleFX::InitExecution()
{
	Super::InitExecution();

	bSteadyState = ((ParticlesMax == 0) && (Distribution != DIST_Uniform));

	if( bSteadyState && (bPrime || Level->TimeSeconds == 0.0) )
		// If systems exist at level startup, or are marked bPrime,
		// give them sufficient retroactive life to reach their steady state.
		if (Age < MaxLifetime())
			Age = MaxLifetime();
	ElapsedTime = 0.f;
}

//----------------------------------------------------------------------------

void AParticleFX::PostEditChange()
{
#if 0 
	AParticleFX* Defaults  = (AParticleFX*)GetClass()->GetDefaultObject();
	Defaults->AngularSpreadWidth.Base = Clamp(Defaults->AngularSpreadWidth.Base, 0.f, 180.f); 
	AngularSpreadWidth.Max = Clamp(AngularSpreadWidth.Max, 0.f, 180.f); 
	AngularSpreadWidth.Rand = Clamp(AngularSpreadWidth.Rand, 0.f, 180.f); 

	AngularSpreadHeight.Base = Clamp(AngularSpreadHeight.Base, 0.f, 180.f); 
	AngularSpreadHeight.Max = Clamp(AngularSpreadHeight.Max, 0.f, 180.f); 
	AngularSpreadHeight.Rand = Clamp(AngularSpreadHeight.Rand, 0.f, 180.f); 

	if (( Chaos > 0.f )&&(ChaosDelay == 0.f))
		ChaosDelay = 0.25f;
#endif
}

//----------------------------------------------------------------------------

UPrimitive* AParticleFX::GetPrimitive() const
{
	return Super::GetPrimitive();
}

//----------------------------------------------------------------------------

inline void ExpandBy( FBox& Box, const FVector& V )
{
	FVector Min = Box.Min + V,
			Max = Box.Max + V;
	Box += Min;
	Box += Max;
}

inline void ExpandBy( FBox& Box, const FBox& B )
{
	FVector Min = Box.Min + B.Min,
			Max = Box.Max + B.Max;
	Box += Min;
	Box += Max;
}

FCoords AParticleFX::GetRenderBoundingBox( UBOOL Exact )
{
	guard(AParticleFX::GetRenderBoundingBox);
	if( (Location - LastUpdateLocation).SizeSquared() > 
		Max( (ParticleList->WorldBox.Max - ParticleList->WorldBox.Min).SizeSquared(), 10000.f ) )
	{
		if( Update() && ParticleList )
			return ParticleList->WorldBox.GetCoords();
		else
			return FCoords(0);
	}

	FBox Box = ParticleList->WorldBox;
	if( Age > ElapsedTime )
	{
		// Evolve the box by an overestimate of particle/system movement.
		// To do: Scripting, bVelocityRelative, Attraction, Damping, Wind, Pattern/Mesh, Drip.

		float StartSize = Max( MaxVal(SizeWidth), MaxVal(SizeLength) ) * 0.5f;
		bool bConst = bSteadyState && Location == LastUpdateLocation;
		//if( !bConst || !Box.IsValid )
		{
			// Add new particle emissions, based on source geometry.
			FBox SourceBox;
			if( Distribution == DIST_OwnerMesh && Owner && Owner->Mesh )
			{
				SourceBox = FBox( Owner->Mesh->GetRenderBoundingBox(Owner, 0) );
			}
			else if( Pattern )
			{
				// Assume pattern points normalised between 0 and 1.
				SourceBox = FBox( FVector(0.f, 0.5f, 0.5f) * DrawScale );
				SourceBox = FBox(SourceBox, ToLocal());
			}
			else
			{
				SourceBox = FBox( FVector( MaxVal(SourceDepth), MaxVal(SourceWidth), MaxVal(SourceHeight) ) * 0.5f );
				SourceBox = FBox(SourceBox, ToLocal());
			}

			// Size.
			Box += SourceBox.ExpandBy(StartSize);
		}

		if( !bConst || ElapsedTime < MaxVal(Lifetime) )
		{
			// Evolve existing particle box.
			float Delta = Min(Age-ElapsedTime, MaxVal(Lifetime));

			// Travel. To do: spread?
			float MaxSpeed = MaxVal(Speed);
			if( ChaosDelay > 0.f )
				MaxSpeed += Chaos/ChaosDelay;
			float Expand = MaxSpeed * Delta;
			float SizeDel = StartSize * Max( MaxVal(SizeEndScale), 0.f ) - 1.f;
			if( Delta < MinVal(Lifetime) )
				SizeDel *= Delta/MinVal(Lifetime);
			Expand += SizeDel;
			Box = Box.ExpandBy( Expand * 1.1f );

			// Gravity.
			AZoneInfo* Info = Region.Zone ? Region.Zone : Level;
			FVector GravityTravel = Info->ZoneGravity * GravityModifier + Gravity;
			GravityTravel *= Square(Delta) * 0.5f;
			ExpandBy( Box, GravityTravel * 1.1f );
		}
	}

	return Box.GetCoords();
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::CreateParticleList()
{
	guard(AParticleFX::CreateParticleList);
	check(!ParticleList);
	check(DrawType == DT_Particles);

	if (bDeleteMe)
		return;
	ParticleList = UParticleList::Create(this);
	check(ParticleList);
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::Destroy()
{
	guard(AParticleFX::Destroy);

	// Kill the particle list.
	delete ParticleList;
	ParticleList = 0;
	Super::Destroy();
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::UpdateParticles(FLOAT Tick)
{
	guard(AParticleFX::UpdateParticles);

	// Quickly destroy particles that will not survive this step.
	FLOAT MaxLife = MaxLifetime();
	if (MaxLife > 0.f && Tick > MaxLife)
	{
		// Destroy all at once.
		ParticleList->RemoveAll();
		return;
	}

	for (UParticle* pparticle = ParticleList->StartParticle(); pparticle; pparticle = ParticleList->NextParticle())
	{
		if ( pparticle->Lifetime > 0.0f && pparticle->Age + Tick >= pparticle->Lifetime )
			ParticleList->RemoveCurrent();
	}
	if ( ParticleList->Size() == 0 ) 
		return;

	// Update surviving particles.
	ULevel* Level = GetLevel();
	
	AZoneInfo* Info = Region.Zone ? Region.Zone : this->Level;
		
	check( Info );

	//
	//	V = Vf - (Vf-Vo) * e^(-dt)
	//	X = Vf * t + ( (Vf-Vo)/d ) * e^(-bt) + Xo
	//

	// Compute constant motion params.
	FVector SystemGravity = Info->ZoneGravity * GravityModifier + Gravity;
	FVector SystemWind(0.0f);
	//fix in case Wind starts to be very dynamic, we should store last wind value and average ?
	if ( Damping * WindModifier > 0.0f ) 
		SystemWind = WindModifier * AWind::GetTotalWind(Level, Location);

	UBOOL bTimeSlice = !Attraction.IsZero() || (Elasticity > KINDA_SMALL_NUMBER);

	FLOAT SystemDamping;
	FLOAT TickSlice;
	while ( Tick > 0.0f ) 
	{
		if ((Tick > MAX_TIMESLICE) && bTimeSlice) 
			TickSlice = MAX_TIMESLICE;
		else
			TickSlice = Tick;

		if ( Damping > 0.0f )
			// Damping = e^(-d t)
			SystemDamping = (float)appExp(-Damping*TickSlice);
		else
			// Damping doesn't effect particles at all
			SystemDamping = 1.0f;

		// Move each particle.
		for (UParticle* pparticle = ParticleList->StartParticle(); pparticle; pparticle = ParticleList->NextParticle())
			pparticle->Update( SystemGravity, SystemDamping, SystemWind, Level, TickSlice, this);

		Tick -= TickSlice;
	}

	unguard;
}

inline FColor operator* (FColor C, float f)
{
	return FColor
	(
		(BYTE)Clamp( appRound(C.R * f), 0, 255 ),
		(BYTE)Clamp( appRound(C.G * f), 0, 255 ),
		(BYTE)Clamp( appRound(C.B * f), 0, 255 ),
		(BYTE)Clamp( appRound(C.A * f), 0, 255 )
	);
}

inline FColor operator+ (FColor A, FColor B)
{
	return FColor
	(
		(BYTE)Clamp( A.R + B.R, 0, 255 ),
		(BYTE)Clamp( A.G + B.G, 0, 255 ),
		(BYTE)Clamp( A.B + B.B, 0, 255 ),
		(BYTE)Clamp( A.A + B.A, 0, 255 )
	);
}

//----------------------------------------------------------------------------

//----------------------------------------------------------------------------

int AParticleFX::EmitParticles(FLOAT Tick)
{
	guard(AParticleFX::EmitParticles);
	Clock(GStat.ParticleEmitTime);

	EmitDelay += Tick;

	FVector EmitDelta = LastEmitLocation - Location;

	FLOAT Emissions;

	AParticleFX* ParentFX = (AParticleFX*)GetClass()->GetSuperClass()->Defaults.GetData();

	// Determine how many particles we want to create.
	Emissions = ParticlesPerSec.Base + appFrand()*ParticlesPerSec.Rand;
	if( ParentBlend != 0.f )
		Emissions += (ParentFX->ParticlesPerSec.Base + appFrand()*ParentFX->ParticlesPerSec.Rand - Emissions) * ParentBlend;
	if ( Distribution != DIST_Uniform )
	{
		//fix just a test.  Trying to find an adequate relationship between particle emission LOD 
		// and particle rendering LOD. LOD for emission is not as extreme as rendering 1) because 
		// most of the time, rendering is more costly 2) the players sees less "Catching Up" do to 
		// LOD changes. There will be some popping, but I think this is better than having to wait 
		// for the system to emit enough particles to fill in an adequate volume.
		Emissions *= Clamp(LOD*3.0f, 0.1f, 1.0f);

		// current emmisions (dampened by LODBias) with residue and elapsed time considered
		Emissions *= EmitDelay;
	}
	else if( Pattern )
	{
		// Compute density based on mid-point of range.
		float F = (Period.Base + Period.Rand*0.5f) * (Pattern->Points.Num()-1);
		int N = Min( int(F), Pattern->Points.Num()-2 );
		FVector Del = Pattern->Points(N) - Pattern->Points(N+1);
		Emissions = Del.Size() * DrawScale * Period.Rand * (Pattern->Points.Num()-1) / Emissions;
	}
	else
	{
		// for DIST_Uniform, ParticlesPerSec is really distance between particles
		//FIX what to do with LOD in this case ?  it may look bad since it would change the gap between particles ?
		Emissions = EmitDelta.Size() / Emissions;
	}

	Emissions += EmissionResidue;

	FLOAT EmitStart = -EmitDelay;

	// Clamp emit time and particle number based on max particle count and lifetime.
	if( ParticlesMax > 0 && Emissions >= (ParticlesMax-ParticlesEmitted) )
	{
		EmitDelay *= (FLOAT)(ParticlesMax-ParticlesEmitted)/Emissions;
		Emissions = (float)(ParticlesMax-ParticlesEmitted);
	}

	// Emit only particles that will live during the interval.
	FLOAT MaxLife = MaxLifetime();
	if( EmitStart < -MaxLife )
	{
		FLOAT NewDelay = EmitDelay + EmitStart + MaxLife;
		Emissions *= NewDelay / EmitDelay;
		EmitDelay = NewDelay;
		EmitStart = -MaxLife;
	}

	// cast float to int to see what we are really going to use
	INT NewParticles = INT(Emissions);

	// save what is not used for next emmision
	EmissionResidue = Emissions - (FLOAT)NewParticles;
	
	if( NewParticles <= 0 )
		return 0;

	FParams NewParams; 

	// Get Transform to WorldSpace.
	FCoords to_world = GMath.UnitCoords * Location * Rotation;

	// ParticlesAlive is the max that can be alive at any moment in time
	// if this is going to be exceeded this emission, pull the oldest ones off the list to make room 
	if ( (ParticlesAlive > 0 ) && (NewParticles + ParticleList->Size() > ParticlesAlive) )
	{
		// if we want to create more than is even allowed, truncate to acceptable value
		if ( NewParticles > ParticlesAlive )
			 NewParticles = ParticlesAlive;

		// computes excess particles ( how many to remove ) ?
		INT Delta = NewParticles + ParticleList->Size() - ParticlesAlive;
		
		// typically the oldest particles are in the front of the 'list'
		for (INT i=0; i<Delta; i++ ) 
			ParticleList->RemoveFromFront();
	}

	// Compute constant gravity and wind params.
	AZoneInfo* Info = Region.Zone ? Region.Zone : Level;
	check( Info );

	FVector SystemGravity = Info->ZoneGravity * GravityModifier + Gravity;
	
	FLOAT SystemDamping = 1.0f;
	FVector SystemWind(0.0f);

	if ( Damping * WindModifier > 0.0f ) 
		SystemWind = WindModifier * AWind::GetTotalWind(GetLevel(), Location);

	TArray<FVector> Verts;
	if( Distribution == DIST_OwnerMesh && Owner && Owner->Mesh )
	{
		// Get world verts for mesh distribution.
		Verts.Add( Owner->Mesh->FrameVerts );
		Owner->Mesh->GetFrame( Verts.GetData(), sizeof(FVector), GMath.UnitCoords, Owner );
	}

	// Get current system params.
	char Buffer[sizeof(AParticleFX)];
	const AParticleFX* SysParams = GetSysParams(Buffer);

	// Add a few new random particles.
	for (int i = 0; i < NewParticles; ++i)
	{
		// get current params depending on "strength" of system
		SysParams->GetParams(NewParams);

		// Delta is a percentage of the elapsed time / distance since the last emission
		FLOAT Delta;
		if( Distribution == DIST_Uniform ) 
		{
			// Evenly space particles.
			if( Emissions > 0 ) 
				Delta = (i+1) / Emissions;
			else
				Delta = 0.f;
		}
		else
			// Randomly place the particles with respect to time and distance since last emission.
			Delta = appFrand();

		// Create a particle.
		UParticle Particle;

		Particle.Lifetime = NewParams.Lifetime;
		FLOAT TimeDelta = -EmitStart - EmitDelay*Delta;
		if( TimeDelta >= Particle.Lifetime )
			continue;

		FVector WorldDir;

		if( Distribution == DIST_OwnerMesh && Owner && Owner->Mesh )
		{
			// Generate particle at a random mesh point.
			INT TV[3];
			INT Tri = appRandRange( 0, Owner->Mesh->GetNumTris()-1 );
			Owner->Mesh->GetTriVerts( Tri, TV );

			// Interpolate a random point.
			FLOAT U = appFrand(), V = appFrand();
			Particle.Position = Verts(TV[0]) * (1.f-U) + Verts(TV[1]) * (U*(1.f-V)) + Verts(TV[2]) * (U*V);

			// Base direction is always the tri normal.
			WorldDir = ( (Verts(TV[1]) - Verts(TV[0])) ^ (Verts(TV[2]) - Verts(TV[0])) ).SafeNormal();
		}
		else 
		{
			if( Pattern )
			{
				// Generate particle at a random pattern point.
				// Rotate it to match gesture convention.
				float F = (Period.Base + Delta*Period.Rand) * (Pattern->Points.Num()-1);
				int N = Min( int(F), Pattern->Points.Num()-2 );
				F -= float(N);
				FVector Pos = Pattern->Points(N) * (1.f-F) + Pattern->Points(N+1) * F;
				Pos = FVector(0.f, Pos.X-0.5f, 0.5f-Pos.Y) * DrawScale;
				Particle.Position = Pos.TransformPointBy(to_world);
			}
			else
			{
				Particle.Position = Location;
			}

			// Calculate rotation variations in terms of radians
			FLOAT WidthTheta = DegToRad(NewParams.AngularSpreadWidth);
			FLOAT WidthVariance = appFrand( -WidthTheta, WidthTheta );

			FLOAT HeightTheta = DegToRad(NewParams.AngularSpreadHeight);
			FLOAT HeightVariance = appFrand( -HeightTheta, HeightTheta );

			// X Axis Rotation is not used now
			// Y Axis rotation = HeightTheta
			// Z Axis rotation = WidthTheta  

			// precompute CosHeight since it used twice below.  minimal.
			float CosHeight = GMath.CosFloat(HeightVariance);

			// compute random variation in direction
			FVector LocalRot( GMath.CosFloat(WidthVariance)*CosHeight, GMath.SinFloat(WidthVariance)*CosHeight, GMath.SinFloat(HeightVariance) ); 

			// Transform to World Rotation
			WorldDir = LocalRot.TransformVectorBy(to_world);
		}

		// calculate random positional offsets in local space
		FVector Pos (	appFrand( -NewParams.SourceDepth*0.5f,  NewParams.SourceDepth*0.5f ),
						appFrand( -NewParams.SourceWidth*0.5f,  NewParams.SourceWidth*0.5f ),
						appFrand( -NewParams.SourceHeight*0.5f, NewParams.SourceHeight*0.5f ));
	
		// Transform local space offsets to World Space
		Particle.Position += Pos.TransformVectorBy(to_world);

		// To do: what about rotation interpolation?
		Particle.Position += EmitDelta*(1.f-Delta);

		// Size the particle to specified dimensions
		Particle.Width = Particle.WidthFull = NewParams.SizeWidth;
		Particle.Length = Particle.LengthFull = NewParams.SizeLength;
		if( SizeGrowPeriod > 0.f )
			Particle.Width = Particle.Length = 0.f;
	
		// Give it a Lifetime.  Hmm, particles with a valid lifetime value know when they will die
		Particle.Lifetime = NewParams.Lifetime;
		
		Particle.DripTimer = NewParams.DripTime;

		if ( Particle.DripTimer == 0.0f )
		{
			// compute a Width and Length delta that will change the particle's size in units per second
			if ( Particle.Lifetime > SizeDelay )
			{
				Particle.WidthDelta = (NewParams.SizeWidth * NewParams.SizeEndScale - NewParams.SizeWidth) / (Particle.Lifetime-SizeDelay);
				Particle.LengthDelta = (NewParams.SizeLength * NewParams.SizeEndScale - NewParams.SizeLength) / (Particle.Lifetime-SizeDelay);
			}
			else
			{
				Particle.WidthDelta = 0.0f;
				Particle.LengthDelta = 0.0f;
			}
		}
		else
		{
			//fix DripTimer and SizeDelay = ?
			Particle.WidthDelta = Particle.Width / Particle.DripTimer;
			Particle.LengthDelta = Particle.Length / Particle.DripTimer;

			Particle.Width = Particle.Length = 0.0f;
		}

		// Calculate Velocity as Unit Direction Vector * Scalar Speed 
		Particle.Velocity = WorldDir * NewParams.Speed;

		// make the velocities of these particles relative to owner
		if ((bVelocityRelative)&&(Owner))
		{
			Particle.Velocity += Owner->Velocity;
		}

		Particle.Color = NewParams.ColorStart.Plane();
		Particle.Alpha = Particle.AlphaFull = NewParams.AlphaStart;
		if( AlphaGrowPeriod > 0.f )
			Particle.Alpha = 0.f;

		Particle.SpinRate = NewParams.SpinRate;
		
		// random orientation for Shards
		if ( RenderPrimitive == PPRIM_Shard ) 
		{
			Particle.Spin = appFrand()*PI*2.0f;
		}

		// use an Unreal palette to color the particle
		if (( ColorPalette ) && ( ColorPalette->Palette ))
		{
			Particle.Color = ColorPalette->Palette->Colors(0).Plane();
		}
		else
		{
			//FIX same issue as above, this can lose precision or end up at incorrect value ( slightly off )
			if( Particle.Lifetime > ColorDelay )
			{
				Particle.ColorDelta.X = ((NewParams.ColorEnd.R - NewParams.ColorStart.R)/255.0f) / (Particle.Lifetime-ColorDelay);
				Particle.ColorDelta.Y = ((NewParams.ColorEnd.G - NewParams.ColorStart.G)/255.0f) / (Particle.Lifetime-ColorDelay);
				Particle.ColorDelta.Z = ((NewParams.ColorEnd.B - NewParams.ColorStart.B)/255.0f) / (Particle.Lifetime-ColorDelay);
				Particle.ColorDelta.W = 0.0f;
			}
			else
				Particle.ColorDelta = FVector(0.f);

			if( Particle.Lifetime > AlphaDelay )
				Particle.AlphaDelta = 	(NewParams.AlphaEnd - NewParams.AlphaStart) / (Particle.Lifetime-AlphaDelay);
			else
				Particle.AlphaDelta = 	0.0f;
		}
		
		FLOAT TickSlice;
		
		UBOOL bTimeSlice = !Attraction.IsZero() || (Elasticity > KINDA_SMALL_NUMBER);
		
		while ( TimeDelta > 0.0f ) 
		{

			if ((TimeDelta > MAX_TIMESLICE) && bTimeSlice) 
				TickSlice = MAX_TIMESLICE;
			else
				TickSlice = TimeDelta;

			if ( Damping > 0.0f )
				// Damping = e^(-d t)
				SystemDamping = (float) appExp(-Damping*TickSlice);
			else
				// Damping doesn't effect particles at all
				SystemDamping = 1.0f;

			// Update this particle.
			Particle.Update( SystemGravity, SystemDamping, SystemWind, GetLevel(), TickSlice, this);

			TimeDelta -= TickSlice;
		}

		CurrentPriorityTag++;
		if (CurrentPriorityTag > 9)
			CurrentPriorityTag = 0;

		// look up into our distribution to keep the priorities pretty evenly distributed
		Particle.PriorityTag = PriorityTable[CurrentPriorityTag]; //appFrand()*10;

		// Add this instance to our list of particles
		ParticleList->Add(Particle); 
	}

	// if we emitted particles update emission maintenance variables
	if ( NewParticles > 0 ) 
	{
		EmitDelay = 0;
		LastEmitLocation = Location;
		ParticlesEmitted += NewParticles;
		STAT(GStat.ParticlesEmitted += NewParticles);
	}

	// I don't use return value currently
	return NewParticles;

	unguard;
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::MaxLifetime()
{
	// Calculate max possible lifetime of any particle in the system.
	return MaxVal(Lifetime);
}

//----------------------------------------------------------------------------

UBOOL AParticleFX::Tick( FLOAT DeltaSeconds, ELevelTick TickType )
{
//	STAT(GStat.SystemsAlive++);

	if( TickType != LEVELTICK_ViewportsOnly || GIsEditor)
	{
		// Do not age until we have, or will have, particles.
		// This prevents scripted systems from ageing before being turned on.
		if( (ParticleList && ParticleList->Size() > 0) || MaxVal(ParticlesPerSec) > 0.f )
		{
			// Age the system.
			Age += DeltaSeconds;

			// Check system lifetime, and prune dead systems in a timely manner.
			if( !GIsEditor && ParticlesMax > 0 && ParticlesPerSec.Base > 0.f )
			{
				FLOAT SysLife = ParticlesMax / ParticlesPerSec.Base + MaxLifetime();
				if( Age > SysLife )
					GetLevel()->DestroyActor( this );
			}
		}
	}

	return Super::Tick(DeltaSeconds, TickType); 
}

//----------------------------------------------------------------------------

bool AParticleFX::Update(FLOAT Step)
{
	guard(AParticleFX::Update);

	if (!ParticleList)
	{
		return false;
	}

	STAT(clock(GStat.ParticleUpdateTime));
	if( Owner && Base && (Owner == Base) && (Owner->GetFlags() & RF_Transient) )
		SetFlags( RF_Transient );

	// First time in, we need to synchronize LastUpdateLocation with current Location.
	if( ElapsedTime == 0.0f ) 
	{
		LastUpdateLocation	= Location;
		LastEmitLocation	= Location;
		LastUpdateRotation	= Rotation;
	}

	if( Step == 0.0f )
		Step = Age - ElapsedTime;

	// Age the system relative to the last time we updated it.
	ElapsedTime += Step;

	if( bSteadyState && Location == LastUpdateLocation && Step > MAX_TIMESLICE )
	{
		// Limit the step time, as accurate offscreen updating not needed.
		float MaxLife = MaxLifetime();
		float MaxRate = MaxVal(ParticlesPerSec);
		float MaxParticles = MaxRate * MaxLife;
		if( ParticlesAlive > 0 )
			MaxParticles = Min( MaxParticles, (float)ParticlesAlive );

		// Take account of current particle count, to allow "priming".
		float MaxStep = (MaxParticles - ParticleList->Size()) / MinVal(ParticlesPerSec);
		MaxStep = Max( MaxStep, MAX_TIMESLICE );
		Step = Min( Step, MaxLife );
		Step = Min( Step, MaxStep );
	}

	// we are probably paused
	if (( Step <= 0.0f )&&( bShellOnly == false )) //fix need flag for bShellOnly or similar ?
	{
		STAT(unclock(GStat.ParticleUpdateTime));
		return true;
	}
	
	//fix new flag for autotick ?  shell doesn't tick actors, game is paused ( single player )
	if ( bShellOnly ) 
	{
		//DOUBLE NewTime = appSeconds();
		//Engine->Tick( NewTime - OldTime );
		//	OldTime = NewTime;

		Step = 0.1f; //fix hack
	}

	// Update all existing particles.
	UpdateParticles(Step);

	STAT(unclock(GStat.ParticleUpdateTime));
	
	if( ParticlesMax == 0 || ParticlesEmitted < ParticlesMax )
	{
		// Create new particles as necessary.
		if( bEmit )
			EmitParticles(Step);
		else
			// Reset.
			LastEmitLocation = Location;
	}
	else
	{
		// Wait for system shutdown.
		if( !ParticleList->Size() )
		{
			if( GIsEditor )
			{
				// Reset the system.
				ParticlesEmitted = 0;
				EmitDelay = 0.f;
			}
			else
			{
				GetLevel()->DestroyActor(this);
				return false;
			}
		}
	}

	ParticleList->UpdateBox();

	LastUpdateLocation = Location;
	LastUpdateRotation = Rotation;
	
	return true;

	unguard;
}

//----------------------------------------------------------------------------

const AParticleFX* AParticleFX::GetSysParams(char* Buffer) const
{
	guard(AParticleFX::GetSysParams);

	if( ParentBlend <= 0.0f )
		return this;

	AParticleFX* ParentFX = (AParticleFX*)GetClass()->GetSuperClass()->Defaults.GetData();
	if( ParentBlend >= 1.f )
		return ParentFX;

	// Blend parent params.
	AParticleFX* NewFX = (AParticleFX*)Buffer;
	appMemcpy(NewFX, this, sizeof(*this));

	FLOAT InvBlend = 1.f - ParentBlend;

	#define BlendValue(p)	\
		NewFX->p = NewFX->p * InvBlend + ParentFX->p * ParentBlend

	#define BlendParam(p)	\
		BlendValue(p.Base);	\
		BlendValue(p.Rand);

	BlendParam(AngularSpreadWidth);
	BlendParam(AngularSpreadHeight);

	BlendParam(SourceWidth);
	BlendParam(SourceDepth);
	BlendParam(SourceHeight);

	BlendParam(Speed);
	BlendParam(Lifetime);

	BlendParam(AlphaStart);
	BlendParam(AlphaEnd);

	BlendParam(SpinRate);
	BlendParam(DripTime);
	
	BlendParam(ColorStart);
	BlendParam(ColorEnd);

	BlendParam(SizeWidth);
	BlendParam(SizeLength);
	BlendParam(SizeEndScale);

	return NewFX;
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::GetParams(FParams& Params) const
{
	guard(AParticleFX::GetParams);

	#define RandValue(p)	\
		Params.p = p.Rand != 0.f ? (p.Base + p.Rand * appFrand()) : p.Base

	RandValue(AngularSpreadWidth);
	RandValue(AngularSpreadHeight);

	RandValue(SourceWidth);
	RandValue(SourceDepth);
	RandValue(SourceHeight);

	RandValue(Speed);
	RandValue(Lifetime);

	RandValue(AlphaStart);
	RandValue(AlphaEnd);

	RandValue(SpinRate);
	RandValue(DripTime);

	if( ColorStart.Rand.IntValue() )
		Params.ColorStart = ColorStart.Base + ColorStart.Rand * appFrand();
	else
		Params.ColorStart = ColorStart.Base;	

	if( ColorEnd.Rand.IntValue() )
		Params.ColorEnd = ColorEnd.Base + ColorEnd.Rand * appFrand();
	else
		Params.ColorEnd = ColorEnd.Base;	

	// Grow size uniformly.
	FLOAT SizeRand = appFrand();
	Params.SizeWidth = SizeWidth.Base + SizeRand*SizeWidth.Rand;
	Params.SizeLength = SizeLength.Base + SizeRand*SizeLength.Rand;

	RandValue(SizeEndScale );

	// Validate final values.
	/*
	Params.AngularSpreadWidth =		Clamp(Params.AngularSpreadWidth, 0.0f, 180.0f);
	Params.AngularSpreadHeight =	Clamp(Params.AngularSpreadHeight, 0.0f, 180.0f);

	Params.SourceWidth =	Clamp(Params.SourceWidth, 0.0f, 10000.0f);
	Params.SourceDepth =	Clamp(Params.SourceDepth, 0.0f, 10000.0f);
	Params.SourceHeight =	Clamp(Params.SourceHeight, 0.0f, 10000.0f);

	Params.Speed =		Clamp(Params.Speed, 0.0f, 1000.0f);
	Params.Lifetime =	Clamp(Params.Lifetime, 0.0f, 100000.0f); // 86400 seconds = 24 hours

	Params.AlphaStart = Clamp(Params.AlphaStart, 0.0f, 1.0f);
	Params.AlphaEnd =	Clamp(Params.AlphaEnd, 0.0f, 1.0f);
	Params.SpinRate =	Clamp(Params.SpinRate, -50.f, 50.0f);
	Params.DripTime =	Clamp(Params.DripTime, 0.0f, 10.0f);

	Params.ColorStart.R = Clamp(Params.ColorStart.R, BYTE(0), BYTE(255));
	Params.ColorStart.G = Clamp(Params.ColorStart.G, BYTE(0), BYTE(255));
	Params.ColorStart.B = Clamp(Params.ColorStart.B, BYTE(0), BYTE(255));

	Params.ColorEnd.R = Clamp(Params.ColorEnd.R, BYTE(0), BYTE(255));
	Params.ColorEnd.G = Clamp(Params.ColorEnd.G, BYTE(0), BYTE(255));
	Params.ColorEnd.B = Clamp(Params.ColorEnd.B, BYTE(0), BYTE(255));

	Params.SizeWidth =		Clamp(Params.SizeWidth, 0.0f, 128.0f);
	Params.SizeLength =		Clamp(Params.SizeLength, 0.0f, 128.0f);
	Params.SizeEndScale =	Clamp(Params.SizeEndScale, 0.0f, 100.0f);
	*/

	unguard;
}

//----------------------------------------------------------------------------

bool AParticleFX::AddParticle( int Id, FVector& Position, FParams* Params )
{
	guard(AParticleFX::AddParticle);

	if ( Params == NULL )
	{
		return false; 
	}

	UParticle Particle(Position);

	// constructor sets to -1 
	Particle.Id = Id;

	// No Rendering LOD scheme for scripted particles
	Particle.PriorityTag = 0;

	// Size the particle to specified dimensions
	Particle.Width = Particle.WidthFull = Params->SizeWidth;
	Particle.Length = Particle.LengthFull = Params->SizeLength;
	if( SizeGrowPeriod > 0.f )
		Particle.Width = Particle.Length = 0.f;

	// Give it a Lifetime.  Hmm, particles with a valid lifetime value know when they will die
	Particle.Lifetime = Params->Lifetime;

	if ( Particle.Lifetime > 0.01f )
	{
		Particle.WidthDelta = (Params->SizeWidth * Params->SizeEndScale - Params->SizeWidth) / Particle.Lifetime;
		Particle.LengthDelta = (Params->SizeLength * Params->SizeEndScale - Params->SizeLength) / Particle.Lifetime;
	}
	else
	{
		Particle.WidthDelta = 0.0f;
		Particle.LengthDelta = 0.0f;
	}

	FCoords to_world = GMath.UnitCoords * LastUpdateLocation * Rotation;

	FLOAT WidthTheta = Params->AngularSpreadWidth*0.01745f;
	FLOAT WidthVariance = appFrand( -WidthTheta, WidthTheta );

	FLOAT HeightTheta = Params->AngularSpreadHeight*0.01745f;
	FLOAT HeightVariance = appFrand( -HeightTheta, HeightTheta );

	// X Axis Rotation is not used now
	// Y Axis rotation = HeightTheta
	// Z Axis rotation = WidthTheta  

	float CosHeight = GMath.CosFloat(HeightVariance);

	FVector LocalRot( GMath.CosFloat(WidthVariance)*CosHeight, GMath.SinFloat(WidthVariance)*CosHeight, GMath.SinFloat(HeightVariance) ); 

	// Transform to World Rotation
	FVector WorldRot = LocalRot.TransformVectorBy(to_world);
	
	// Calculate Velocity as Unit Direction Vector * Scalar Speed 
	Particle.Velocity = WorldRot * Params->Speed;

	Particle.Color = Params->ColorStart.Plane();
	Particle.Alpha = Particle.AlphaFull = Params->AlphaStart;
	if( AlphaGrowPeriod > 0.f )
		Particle.Alpha = 0.f;

	Particle.SpinRate = Params->SpinRate;
	Particle.DripTimer = Params->DripTime;

	if ( Particle.DripTimer > 0.0f )
	{
		Particle.WidthDelta = Particle.Width / Particle.DripTimer;
		Particle.LengthDelta = Particle.Length / Particle.DripTimer;

		Particle.Width = Particle.Length = 0.0f;
	}

	if (( ColorPalette ) && ( ColorPalette->Palette ))
	{
		Particle.Color = ColorPalette->Palette->Colors(0).Plane();
	}
	else
	{
		//FIX same issue as above, this can lose precision or end up at incorrect value ( slightly off )
		if ( Particle.Lifetime > ColorDelay )
		{
			FLOAT LifeReciprocal = 1.0f / (Particle.Lifetime-ColorDelay);
			FLOAT ColorReciprocal = 1.0f / 255.0f * LifeReciprocal;

			Particle.ColorDelta.X = (Params->ColorEnd.R - Params->ColorStart.R) * ColorReciprocal;
			Particle.ColorDelta.Y = (Params->ColorEnd.G - Params->ColorStart.G) * ColorReciprocal;
			Particle.ColorDelta.Z = (Params->ColorEnd.B - Params->ColorStart.B) * ColorReciprocal;
		}
		else
		{
			Particle.ColorDelta.X = 0.0f;
			Particle.ColorDelta.Y = 0.0f;
			Particle.ColorDelta.Z = 0.0f;
		}
		Particle.ColorDelta.W = 0.0f;

		if ( Particle.Lifetime > AlphaDelay+0.001f )
		{
			FLOAT LifeReciprocal = 1.0f / (Particle.Lifetime-AlphaDelay);
			Particle.AlphaDelta = (Params->AlphaEnd - Params->AlphaStart) * LifeReciprocal;
		}
		else
			Particle.AlphaDelta = 0.0f;
	}
	
	// Add this instance to our list of particles
	ParticleList->Add(Particle); 

	return true;

	unguard;
}

//----------------------------------------------------------------------------

int AParticleFX::RecomputeDeltas(INT Id)
{
	if (( ColorPalette )||(!ParticleList))
	{
		return 0;
	}

	UParticle* p = ParticleList->GetParticle(Id);

	if ( !p )
		return 0;

	float fLifeRemaining = p->Lifetime - p->Age;

	if ( fLifeRemaining > 0.0f )
	{
		FLOAT LifeReciprocal=1.0f/fLifeRemaining;
		FLOAT ColorReciprocal=1.0f/255.0f*LifeReciprocal;

		p->ColorDelta.X = ((ColorEnd.Base.R - p->Color.X)*ColorReciprocal);
		p->ColorDelta.Y = ((ColorEnd.Base.G - p->Color.Y)*ColorReciprocal);
		p->ColorDelta.Z = ((ColorEnd.Base.B - p->Color.Z)*ColorReciprocal);
		p->ColorDelta.W = 0.0f;

		p->AlphaDelta = (AlphaEnd.Base - p->Alpha) * LifeReciprocal;

		p->WidthDelta  = (SizeEndScale.Base * p->Width - p->Width) * LifeReciprocal;
		p->LengthDelta = (SizeEndScale.Base * p->Length - p->Length) * LifeReciprocal;
	}
	else
		return 0;

	return 1;

}

//----------------------------------------------------------------------------

FLOAT AParticleFX::Lod( float ScreenFrac )
{
	if( Distribution == DIST_Uniform )
		return 1.0f;
	else
		return Min(1.0f, LODBias * 2.0f * GMath.SqrtApprox(ScreenFrac));
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::LodParticles( float ScreenFrac )
{
	return Lod(ScreenFrac) * (ParticleList ? ParticleList->Size() : 0);
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::LodParticleDensity( FSceneNode* Frame, float ScreenFrac )
{
	if( !ParticleList )
		return 0.0f;

	// Estimate average screen coverage of particle in this system.
	// Use sampling for large systems.
	FLOAT CurLod = Lod(ScreenFrac);
	INT MaxPriorityTag = appRound(CurLod * 9);

	FLOAT SampleLod = 20.f / ParticleList->Size();
	INT SamplePriorityTag = Min( MaxPriorityTag, appRound(SampleLod * 9) );
	FLOAT Factor = (FLOAT)(MaxPriorityTag + 1.0) / (FLOAT)(SamplePriorityTag + 1.0);
	MaxPriorityTag = SamplePriorityTag;

	FLOAT SizeMul = Frame->Proj.Z / (Frame->FX * Frame->FY);
	FLOAT ParticleDensity = 0.0f;
	for (UParticle* pparticle = ParticleList->StartParticle(); pparticle; pparticle = ParticleList->NextParticle())
	{
		if (pparticle->PriorityTag > MaxPriorityTag)
			continue;

		FLOAT Dist = (pparticle->Position - Frame->Coords.Origin).SizeApprox();
		FLOAT Size = pparticle->Width * pparticle->Length * SizeMul;
		if( Size >= Dist )
			ParticleDensity += 1.0f;
		else
			ParticleDensity += Size/Dist;
	}

	return ParticleDensity * Factor;
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::ParticleLOD = 1.0f;

//----------------------------------------------------------------------------

void AParticleFX::execNumParticles( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execNumParticles);
	P_FINISH;

	if (ParticleList)
		*(INT*)Result = ParticleList->Size();
	else
		*(INT*)Result = -1;
	unguardexec;
}

//----------------------------------------------------------------------------

void AParticleFX::execAddParticle( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execAddParticle);
	P_GET_INT(Id);
	P_GET_VECTOR(Loc);
	P_FINISH;

	// Scripting invalidates steady-state optimisation.
	bSteadyState = false;

	char Params[sizeof(AParticleFX)];
	FParams NewParams;
	GetSysParams(Params)->GetParams(NewParams);

	AddParticle(Id, Loc, &NewParams);

	unguardexec;
}

//----------------------------------------------------------------------------

void AParticleFX::execGetParticleParams( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execGetParticleParams);
	P_GET_INT(Id);
	P_GET_STRUCT_REF(FParticleParams,Params)
	P_FINISH;

	UParticle* p = NULL;
	
	if (ParticleList)
		p = ParticleList->GetParticle(Id);

	if ( p )
	{
		Params->Position = p->Position;
		Params->Velocity = p->Velocity;
		Params->Lifetime = p->Lifetime;
		Params->Alpha = p->Alpha;
		Params->Color = p->Color;
		Params->Width = p->Width;
		Params->Length = p->Length;
		Params->DripTimer = p->DripTimer;
		Params->SpinRate = p->SpinRate;

		*(bool*)Result = true;
	}
	else
		*(bool*)Result = false;

	unguardexec;
	
}

//----------------------------------------------------------------------------

void AParticleFX::execSetParticleParams( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execSetParticleParams);
	P_GET_INT(Id);
	P_GET_STRUCT_REF(FParticleParams,Params)
	P_FINISH;

	UParticle* test = NULL;
	
	// Scripting invalidates steady-state optimisation.
	bSteadyState = false;

	if (ParticleList)
		test = ParticleList->GetParticle(Id);
	
	if ( test )
	{
		test->Position = Params->Position ;
		test->Velocity = Params->Velocity ;
		test->Lifetime = Params->Lifetime;
		test->Alpha = Params->Alpha ;
		test->Color = Params->Color.Plane();
		test->Width = Params->Width;
		test->Length = Params->Length;
		test->DripTimer = Params->DripTimer;
		test->SpinRate = Params->SpinRate;
	
		*(bool*)Result = true;
	}
	else
		*(bool*)Result = false;
	

	unguardexec;
	
}

//----------------------------------------------------------------------------

void AParticleFX::execRecomputeDeltas( FFrame& Stack, RESULT_DECL )
{
	P_GET_INT(Id);
	P_FINISH;

	if ( RecomputeDeltas(Id) )
		*(bool*)Result = true;
	else
		*(bool*)Result = false;
}



IMPLEMENT_CLASS(AParticleFX);


/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

