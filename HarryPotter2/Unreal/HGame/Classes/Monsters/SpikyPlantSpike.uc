
// Class Name  : SpikyPlantSpike
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : SpikyPlantSpike AI. The spikes that are thrown off of the spiky plant
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------


class SpikyPlantSpike extends Projectile;

var int Lift;   // opposite of gravity
var int speed;
var vector start_location;
var rotator flight_direction;

var vector prev_location;
var int start_pitch;
var int iDamage;

var		SpikyBushSmoke		Smoke;

auto state flying
{
	function disintegrate ()
	{
//		spawn(class'avifors_hit',,,location,rot(0,0,0));
		destroy ();
	}

	function tick(float deltaTime)
	{
		local vector locn_diff;
		local rotator locn_rot;

		if (Physics==PHYS_Falling)
		{
			locn_diff = location-prev_location;

			if (VSize2d (locn_diff) > 0.5)
			{
				locn_rot = rotator (locn_diff);

				locn_rot.pitch += start_pitch;

				//Log ("time: " $deltatime $", diff: " $locn_diff $", rotation: " $rotation $", locn_rot: " $locn_rot);

				SetRotation (locn_rot);

				prev_location = location;
			}

			velocity.z += Lift * deltaTime;

			Smoke.Move(location - Smoke.location);
		}

//		if (vSize (location-start_location) > Range)
//			disintegrate ();
	}

	function Touch (actor other)
	{
		// momentum = velocity * weight. Assume a weight of 1 until we know otherwise

		if (other.IsA('Harry'))
		{
//			if (baseHud(playerharry.myHud).bCutSceneMode == false)
//			{
				other.TakeDamage (iDamage, Pawn(Owner), location, velocity*1, 'Spiked');
				disintegrate ();
//			}
		}
	}

	function Landed( vector HitNormal )
	{
		disintegrate ();
	}

	function HitWall (vector HitNormal, actor Wall)
	{
		disintegrate ();
	}

	function BeginState()
	{
		disable( 'Tick' );	
//		log("started spike");
		start_location = location;
		prev_location = location;

		flight_direction = rotation;
		flight_direction.pitch -= (65536*3) / 4;

		velocity.x = speed;
		velocity = velocity >> flight_direction;

//		start_pitch = rotation.pitch;
		start_pitch = rotation.pitch+16384;

		Smoke = spawn(class'SpikyBushSmoke', [spawnlocation] location);
		Smoke.SetPhysics(PHYS_Trailer);

		enable( 'Tick' );	
	}
}

defaultproperties
{
	 Physics=PHYS_Falling
     bStatic=False
     Velocity=(X=300)
     Mesh=SkeletalMesh'HPModels.skSpikyPlantSpikeMesh'
     AmbientGlow=65
     CollisionRadius=5
     CollisionHeight=2
	 Lift=400
	 Speed=300
//	 bCanBeThrown=False
}
