//=============================================================================
// FlyingCarHarry  -- hero character, flying in a car
//=============================================================================
class FlyingCarHarry extends BroomHarry;

var vector vCurrentTetherDistance; // The current distance from the path

var() float strafeSpeed;		// The speed the car will strafe.
var() float updownSpeed;		// The speed the car will go up and down.
var() float rollAmount;			// The amount of roll added to the car when it turns.
var() float pitchAmount;		// The amount of pitch added to the car when it goes up/down
var() float yawAmount;			// The amount of yaw added to the car when it turns.
var() float MaxDistanceToSide;	// The Maximum distance to the left or right of the path.
var() float MaxDistanceUpDown;	// The Maximum distance up or down from the path.

var() float AirSpeedNormal;
var() float	AirSpeedBoost;
var() name pathName;			// The name of the path that the guide follows

var rotator guideRotation;		// The rotation of the guide. Mirror this to follow path
var float	sideDistance;		// The distance from the path to the side
var float	upDistance;			// The distance from the path up or down
var FlyingFordPathGuide guide;	// The guide that follows the path
var vector vTurbulence;			// The amount of turbulence on the car
var float fLightningYaw;		// The amount of yaw on the car from a lightning strike
var float fLightningPitch;		// The amount of pitch on the car from a lightning strike

//-------------------------------------------------------------------------------------------
// PreBeginPlay(), PostBeginPlay(), and common events
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

	// Find mini-game director
	foreach AllActors( class'Director', Director )
		break;
}


function PostBeginPlay()
{

	// Initialize Harry
	Super.PostBeginPlay();

	// Setup animation layering
	PrimaryAnim = 'flying';
	SecondaryAnim = '';
	LoopAnim( PrimaryAnim );
	LookForTarget = None;
	bLookingForTarget = false;

	// Force rotation rates (ignore settings in editor)
	fRotationRateYaw = 20000;	// Yaw uses this override because roll is tied to RotationRate.Yaw

	RotationRate.Yaw = 20000;
	RotationRate.Roll = 10000;
	RotationRate.Pitch = 20000;

	vCurrentTetherDistance = vec(0,0,0);

}


event Possess()
{
	// Called when the PlayerPawn Harry is possessed (attached) to a viewport (Player).
	Super.Possess();

	// Make sure the right physics is selected (workaround for bug with
	// LoadGame where physics isn't set correctly)
	Log( "BroomHarry in State "$GetStateName()$"." );

	// Let director know possession has occurred happened
	Director.OnPlayerPossessed();

	SetLocation(guide.location);
	SetRotation(guide.rotation);

}

function DeterminePrimaryAnim()
{

	// Determine which animation is appropriate for current motion
	local float		Speed;

	Speed = VSize(Velocity);
/*
	if ( !bHitWall )
	{
		if ( Rotation.roll > 1000 && Rotation.roll < 0x8000 )
			SetPrimaryAnimation( 'Turn_Right', , 1.0 );
		else if ( Rotation.roll < (0x00010000 - 1000) && Rotation.roll > 0x8000 )
			SetPrimaryAnimation( 'Turn_Left', , 1.0 );
		else if ( Rotation.pitch > 1000 && Rotation.pitch < 0x8000 )
			SetPrimaryAnimation( 'Pull_Up', , 1.0 );
		else if ( Rotation.pitch < (0x00010000 - 1000) && Rotation.pitch > 0x8000 )
			SetPrimaryAnimation( 'Dive', , 1.0 );
		else if ( bBroomBrake != 0 && Deceleration < AirSpeedNormal * 0.48 )
			SetPrimaryAnimation( 'Brake', , 1.0 );
		else if ( (bBroomBoost != 0 || bAuxBoost) && bBroomBrake == 0 )
			SetPrimaryAnimation( 'Boost', , 1.0 );
		else if ( Speed < 50 )
			SetPrimaryAnimation( 'Hover', , 0.4 );
		else
			SetPrimaryAnimation( 'Fly_Forward', , 1.0 );
	}
*/
	// car only has one anim
	SetPrimaryAnimation( 'Flying', , 1.0 );
	Trail.ParentBlend = Min(Speed/200, 1);  
}


function UpdateBroomSound()
{
	// Sets the broom sound pitch and volume to correspond with Harry's motion
	local float		fSpeed;
	local float		fSpeedFactor;
	local float		fTurnFactor;
	local float		fVolume;
	local float		fPitch;

	// Compute the volume and pitch for the broom sound based on speed
	fSpeed = VSize( Velocity );
	if ( fSpeed < 50 )
	{
		fVolume = 0.0;
		fPitch  = 1.5;
	}
	else if ( fSpeed <= AirSpeedNormal )
	{
//		PlaySound(sound'HPSounds.Quidditch_sfx.broom_accel');

		fSpeedFactor = (fSpeed - 50) / (AirSpeedNormal - 50);
		fVolume = 0.6 * fSpeedFactor;
		fPitch  = 0.2 * fSpeedFactor + 1.5;
	}
	else
	{
		fSpeedFactor = (fSpeed - AirSpeedNormal) / (AirSpeedBoost - AirSpeedNormal);
		fVolume = 0.4 * fSpeedFactor + 0.6;
		fPitch  = 0.15 * fSpeedFactor + 1.7;
	}

	// Modify volume and pitch to account for roll and pitch
	if ( Rotation.roll <= 0x8000 )
		fTurnFactor = Rotation.roll / 4096.0;
	else if ( Rotation.roll > 0x8000 )
		fTurnFactor = (0x00010000-Rotation.roll) / 4096.0;

	if ( Rotation.pitch <= 0x8000 )
		fTurnFactor += Rotation.pitch / 8192.0;
	else if ( Rotation.pitch > 0x8000 )
		fTurnFactor += (0x00010000-Rotation.pitch) / 8192.0;

	fTurnFactor *= 0.5;
	if ( fTurnFactor > 1.0 )
		fTurnFactor = 1.0;

	fVolume *= 1.0 + 2.0 * fTurnFactor;
	fPitch  *= 1.0 + 1.0 * fTurnFactor;

	//AE:
	if( fTurnFactor > 0.f && fTurnFactor < 0.1f )
	{
		if( fSpeedFactor > 0.7f )
			PlayFastWhooshSound();
		else
			PlaySlowWhooshSound();
	}

	//AE: Quiet background, dynamic hiss.
	if ( !ModifySound( SOUND_Volume, fTurnFactor, MainBroomSound, SLOT_Interact ) )
		PlaySound( MainBroomSound, SLOT_Interact, fTurnFactor, true, , 1.0 );

	// Change the broom sound parameters
	if ( !ModifySound( SOUND_Volume, fVolume, BroomSound, SLOT_Misc ) )
		PlaySound( BroomSound, SLOT_Misc, fVolume, true, , fPitch );	// Wasn't already playing; start it now
	else
		ModifySound( SOUND_Pitch, fPitch, BroomSound, SLOT_Misc );
}

function vector SideDirection(float fYawControl)
{

	local vector vGuideDirection, vUp, vDown;
	local vector vRight, vLeft;


	vguideDirection = vector(guide.rotation);
	vUp = vec(0,0,1);
	vDown = vec(0,0,-1);

	vLeft = vGuideDirection cross vDown;

	return vLeft;


}



//-------------------------------------------------------------------------------------------
// States
//
// PlayerWalking	- Main state of motion
// CutIdleing		- Captured by a cut-scene script
// FlyingOnPath		- Flying and following a interpolation path; non-interactive
// Pursue			- Chasing target while free-flying
// Hit				- Reacting to a damaging hit
// BroomDying		- Doing his death-spiral after falling off broom
// Catching			- Reaching out and catching an object
//-------------------------------------------------------------------------------------------

state PlayerWalking	// Well, flying actually; but as his normal mode of getting around
{
	ignores AltFire, Mount;

	function BeginState()
	{
		super.BeginState();
		SetPhysics(PHYS_Flying);

		// Find camera and set it into FlyingCar mode
		cam.SetCameraMode( CM_FlyingCar );
	}


	// Overrides regular Harry's rotation behavior to remap controls for
	// flying (using forward controls to control pitch)
	function UpdateRotation( float DeltaTime, float maxPitch )
	{
		local rotator	NewRotation;
		local float		YawVal;
		local float		DeltaYaw;
		local int		nDeltaYaw;
		local float		DeltaPitch;
		local float		fPitchLimitHi;
		local float		fPitchLimitLo;
		local float		fEffectiveMousePitch;
		local vector	MovementDirection;
		local float		CarYawVal;

		NewRotation = Rotation;

//log("owner's prepivot  :  " $owner.PrePivot);

		fPitchLimitHi = PitchLimitUp * (0x4000/90.0);
		fPitchLimitLo = 0x00010000 - (PitchLimitDown * (0x4000/90.0));

		// Modify pitch using current input source
//		if ( bPitchUnderMouse )
		if ( false )
		{
			// Adjust for deadband
			if ( fMousePitch > 0.15 )
			{
				fEffectiveMousePitch = fMousePitch - 0.15;
				if ( fEffectiveMousePitch > 1.0 )
					fEffectiveMousePitch = 1.0;
			}
			else if ( fMousePitch < -0.15 )
			{
				fEffectiveMousePitch = fMousePitch + 0.15;
				if ( fEffectiveMousePitch < -1.0 )
					fEffectiveMousePitch = -1.0;
			}
			else
				fEffectiveMousePitch = 0.0;

			// See if pitch has reversed
			if ( fEffectiveMousePitch < 0.0 && !bLastPitchNeg )
			{
				NoteAnotherReversal();
				bLastPitchNeg = true;
			}
			else if ( fEffectiveMousePitch > 0.0 && bLastPitchNeg )
			{
				NoteAnotherReversal();
				bLastPitchNeg = false;
			}

			// Update pitch
			NewRotation.Pitch = fEffectiveMousePitch * fPitchLimitHi;
			NewRotation.Pitch = NewRotation.Pitch & 0x0000ffff;
		}
		else
		{

			// See if pitch control has reversed
			if ( fPitchControl < 0.0 && !bLastPitchNeg )
			{
				NoteAnotherReversal();
				bLastPitchNeg = true;
			}
			else if ( fPitchControl > 0.0 && bLastPitchNeg )
			{
				NoteAnotherReversal();
				bLastPitchNeg = false;
			}

			// Make pitch self-centering when no pitch adjust is being commanded
			if ( abs( fPitchControl ) < 0.0005 )
			{
				if ( Rotation.Pitch >= 0x8000 )
					DeltaPitch = 0x00010000 - Rotation.Pitch;
				else
					DeltaPitch = Rotation.Pitch;

				fPitchControl = DeltaPitch / (RotationRate.Pitch * DeltaTime);

				if ( fPitchControl > 1.0 )	// Return to horz no faster than commanded rate
					fPitchControl = 1.0;
				if ( Rotation.Pitch < 0x8000 )
					fPitchControl = -fPitchControl;
			}

			// Apply pitch control
			NewRotation.Pitch += RotationRate.Pitch * DeltaTime * fPitchControl;

//			NewRotation.Pitch = NewRotation.Pitch & 0x0000ffff;

			// Limit pitch
			If ( (NewRotation.Pitch > fPitchLimitHi) && (NewRotation.Pitch < fPitchLimitLo) )
			{
				If (fPitchControl > 0) 
					NewRotation.Pitch = fPitchLimitHi;
				else
					NewRotation.Pitch = fPitchLimitLo;
			}

		}


		// See if yaw control has reversed
		if ( fYawControl < 0.0 && !bLastYawNeg )
		{
			NoteAnotherReversal();
			bLastYawNeg = true;
		}
		else if ( fYawControl > 0.0 && bLastYawNeg )
		{
			NoteAnotherReversal();
			bLastYawNeg = false;
		}

		// If hitting wall and not commanding Harry's yaw, turn him out of wall automatically
		if ( abs( fYawControl ) < 0.0005 )
		{
			if ( bHittingWall )
			{
				fYawControl = WallAvoidanceYaw * fWallAvoidanceRate / ( fRotationRateYaw * DeltaTime );
				fLastTimeAvoidedWall = Level.TimeSeconds;
			}
		}
		else
			fLastTimeAvoidedWall = -1.0f;	// Forget Harry was ever trying to avoid wall; player overrode

		bHittingWall = false;

		// Apply yaw control
		if ( fYawControl > 1.0 )
			 fYawControl = 1.0;
		else if ( fYawControl < -1.0 )
			 fYawControl = -1.0;
		YawVal = fRotationRateYaw * DeltaTime * fYawControl;
		if(Acceleration == vect(0,0,0))
			YawVal = 4.0/3.0 * YawVal;

		CarYawVal = 4096 * fYawControl;

		// Update the offset from the path guide
		sideDistance += yawVal/strafeSpeed;

		// up and down is computed from a rotator so it needs to be translated into distance
		// Assume that if the pitch is greater than 32768 (180 degrees) it is going down
		if ( NewRotation.Pitch != 0 )
		{
			if ( NewRotation.Pitch > 32768 )
			{
				upDistance += (NewRotation.Pitch - 65535)/updownSpeed;
			}
			else
			{
				upDistance += (NewRotation.Pitch/updownSpeed);
			}
		}

		// Limit the distance side to side
		if ( sideDistance > MaxDistanceToSide )
		{
			sideDistance = MaxDistanceToSide;
			CarYawVal = 0;
		}
		if (sideDistance < -MaxDistanceToSide )
		{
			sideDistance = -MaxDistanceToSide;
			CarYawVal = 0;
		}

		// Limit the distance up and down
		if ( upDistance > MaxDistanceUpDown )
			upDistance = MaxDistanceUpDown;
		if (upDistance < -MaxDistanceUpDown )
			upDistance = -MaxDistanceUpDown;


		// Update the vector for the offset
		MovementDirection = SideDirection(fYawControl);
		vCurrentTetherDistance.x = sideDistance * MovementDirection.x;
		vCurrentTetherDistance.y = sideDistance * MovementDirection.y;
		vCurrentTetherDistance.z = upDistance;

		vCurrentTetherDistance += vTurbulence;


		SetLocation(guide.location + vCurrentTetherDistance);
		
		NewRotation.Pitch += (pitchAmount*fPitchControl);
		NewRotation.Pitch = NewRotation.Pitch & 0x0000ffff;

		guideRotation = guide.Rotation;
		guideRotation.yaw += CarYawVal;
		guideRotation.roll += CarYawVal;
		guideRotation.pitch += NewRotation.Pitch;

		// separate yaw and pitch addition from lightning strike
		guideRotation.yaw += fLightningYaw;
		guideRotation.pitch += fLightningPitch;


		DesiredRotation = guideRotation;

		if ( airSpeed == AirSpeedNormal )
			guide.IPSpeed = 0;
		else
			guide.IPSpeed = AirSpeedBoost;

	}

	function HitWall( vector HitNormal, actor Wall )
	{
		local Vector	WallFaceDir;
		local Rotator	WallFaceRot;
		local Vector	Up;
		local Vector	FlightDir;
		local float		fSpeed;
		local int		EffectiveDamage;
		local float		fVolume;
		local bool		bTurnToRight;

		// Ignore hits  with flat ceilings (usually an invisible BlockAll on a sky)
		if ( HitNormal.Z < -0.9999 )
			return;

		// If initial hit with wall...
		if ( !bHitWall )
		{
//			ClientMessage("Hit Wall "$Wall.Name$" "$HitNormal);
			bHitWall = true;

			// Play sound of collision, speed dependant
			fSpeed = VSize( Velocity );
			fVolume = fSpeed / AirSpeedNormal;
//			PlaySound( HitSounds[ Rand( NUM_HIT_SOUNDS ) ], SLOT_Interact, fVolume );

			// Take damage, but don't let Harry go into Hit state (thus the skipping over parent class)
			if ( WallDamage > 0 )
			{
//				EffectiveDamage = WallDamage * fSpeed / AirSpeedNormal;
//				if ( EffectiveDamage > 0 )
//					Super(Harry).TakeDamage( EffectiveDamage, Self, Location, 100*Normal(Velocity), 'Collided' );
			}

			// Play bump animation
//			PlayAnim( 'Bump' );

			// Tell the game director when Harry hits things
			Director.OnHitEvent( Self );
		}

		// Compute how fast Harry should try to turn out of wall
		// (but don't try to correct direction at all if surface
		// is more like a floor than a wall)
		if ( abs(HitNormal.Z) >= 0.985 )	// cos(+/- 10 degrees)
			return;
		else
			fWallAvoidanceRate = 1.0 - (abs(HitNormal.Z) / 0.985);	// Scaled by how plumb wall is

		// Compute vector and rotator for face of wall
		Up.x = 0.0f;
		Up.y = 0.0f;
		Up.z = 1.0f;
		WallFaceDir = HitNormal Cross Up;
		WallFaceRot = Rotator(WallFaceDir);

		// Compute change in rotation needed to travel along and away from wall
		FlightDir = Vector(Rotation);

		if (    fLastTimeAvoidedWall != -1.0f
			 && Level.TimeSeconds - fLastTimeAvoidedWall < fMaxTimeSameAvoidDir )
		{
			bTurnToRight = bLastAvoidanceRight;	// Keep turning same direction to avoid sticking in corners
		}
		else
		{
			bTurnToRight = (FlightDir Dot WallFaceDir) >= 0.0;
			bLastAvoidanceRight = bTurnToRight;
		}

		if ( bTurnToRight )
		{
			WallAvoidanceYaw = (WallFaceRot.Yaw + 1000 - Rotation.Yaw) & 0x0000ffff;	// Turn to right plus 10 degrees off wall
			if ( WallAvoidanceYaw > 0x00006000 )
				WallAvoidanceYaw = 0x00006000;		// Limit turn to less than 135 degrees (to avoid turning wrong way)
		}
		else
		{
			WallAvoidanceYaw = (WallFaceRot.Yaw + 0x00008000 - 1000 - Rotation.Yaw) & 0x0000ffff;	// Turn to left plus 10 degrees off wall
			if ( WallAvoidanceYaw < 0x0000A000 )
				WallAvoidanceYaw = 0x0000A000;		// Limit turn to less than 135 degrees (to avoid turning wrong way)
			WallAvoidanceYaw -= 0x00010000;			// Make negative; it'll be used as a delta
		}

		bHittingWall = true;	// Note that Harry's still in contact with wall
	}

	event Timer()
	{
		// Time to do another look-around for target
	//	PlayAnim( 'Look', , 1.0 );
	//	SetTimer( frand() * 4.0 + 1.5, false );		// Play it again a little later
	}

}






defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skFordFlyingMesh'
	bAlignBottom=false
	MaxMountHeight=0.0	// Harry: thou shalt not auto-mount while flying
//	AirSpeedNormal=300
//	AirSpeedBoost=500
	PitchLimitUp=45
	PitchLimitDown=45
	IdleAnimName='Flying'
	RotationRate=(Yaw=150000)
	RotationRate=(Roll=6000)
	RotationRate=(Pitch=24000)
	WallDamage=1

	strafeSpeed=100
	updownSpeed=150
	rollAmount=3
	pitchAmount=9
	yawAmount=8
	MaxDistanceToSide=1000
	MaxDistanceUpDown=75
	AirSpeedNormal=800
	AirSpeedBoost=800
}
