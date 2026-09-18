//=============================================================================
// BroomHarry  -- hero character, on a broom
//=============================================================================
class BroomHarry extends Harry;

var float	fPitchControl;				// Range +1.0 to -1.0 (up/down)
var float	fYawControl;				// Range +1.0 to -1.0 (right/left)
var float	fRotationRateYaw;			// Yaw uses this because Roll is tied to RotationRate.Yaw for some reason

var(Movement) int	AirSpeedNormal;		// How fast Harry flies when neither boosted nor braked
var(Movement) int	AirSpeedBoost;		// How fast Harry flies when boosted

var(Movement) int	PitchLimitUp;		// How high can Harry pitch broom up; degrees
var(Movement) int	PitchLimitDown;		// How low can Harry pitch broom down; degrees

var(Movement) int	WallDamage;			// How much damage Harry takes when he hits walls
var(Movement) float	ArmorDamageScale;	// How to scale taken damage when Harry owns quidditch armor

var StatusItemQArmor	QuidArmorStatus;// Inventory Item for Quidditch Armor

var bool	bInvincible;				// Harry takes all damage as zero (suffers no health loss)

var bool	bAuxBoost;					// Non-interactive Boost command that can come from game script
var	int		Deceleration;

var bool	bHasEverBoosted;			// Has the boost button ever been pressed
var bool	bHasEverBraked;				// Has the brake button ever been pressed

var bool	bActioned;					// Harry was pressed the action key this tick
var bool	bWasActioning;				// Harry was pressing the action key last tick

var float	fMousePitch;				// Cumulative Pitch from mouse axis motion
var float	fMouseYaw;					// Cumulative Yaw from mouse axis motion
var float	fBroomSensitivity;			// Scalar on Broom axis inputs (mouse)

//
//	Devices that can control broom inputs
//
enum EControlDevice
{
	DEVICE_Button,						// e.g. Keyboard
	DEVICE_Mouse,						// Mouse
	DEVICE_Joystick,					// Analog joystick
	DEVICE_Gamepad						// Digital joystick/gamepad
};

var EControlDevice	eYawControlDevice;	// device controlling yaw
var EControlDevice	ePitchControlDevice;// device controlling pitch

var bool	bLastYawNeg;				// Last change in yaw was in negative direction (for statistics)
var bool	bLastPitchNeg;				// Last change in pitch was in negative direction (for statistics)
var bool	bHit;						// Reacting to a bump with actor
var bool	bHitWall;					// Reacting to initial collision with wall
var bool	bHittingWall;				// In contact with wall this tick
var int		WallAvoidanceYaw;			// How far Harry should turn to avoid wall (while hitting it)
var float	fWallAvoidanceRate;			// How fast Harry should turn to avoid wall (while hitting it)
var float	fLastTimeAvoidedWall;		// When wall avoidance was last applied
var bool	bLastAvoidanceRight;		// Whether last avoidance rotation was toward the right
const		fMaxTimeSameAvoidDir = 1.0f;	// How long Harry can go between wall hits before clearing current avoidance direction trend

var float	fTimeForNextError;			// When next to change the target error offset
var vector	TargetError;				// Error offset from target to point actually pursued
var float	fTargetTrackDist;			// How far back from target should Harry try to maintain position
var float	fTargetTrackHorzOffset;		// How far left(-) or right(+) of center line should Harry track the target
var float	fTargetTrackVertOffset;		// How far below(-) or above(+) center line should Harry track the target
var	float	TrackingOffsetRange_Horz;	// Maximum horizontal offset Harry can track target with
var	float	TrackingOffsetRange_Vert;	// Maximum vertical offset Harry can track target with

var name	PrimaryAnim;				// What underlying broom animation is Harry looping right now
var name	SecondaryAnim;				// What overlay animation is Harry looping right now

var Sound	BroomSound;					// The sound of broom in flight
var Sound	MainBroomSound;				// The sound of broom in flight

const		NUM_HIT_SOUNDS = 3;
var Sound	HitSounds[3];				// All the different sounds of Harry hitting an obstacle

var InterpolationManager		IM;
var DynamicInterpolationPoint	TransPath[2];	// Transitional path to follow to get to main path
var int							iTransPoint;	// Position Id of point on main path that player is transitioning to

var string	CutCommandCue;				// Cue to signal at end of a commandable state

var ParticleFX	Trail;

var Actor	LookForTarget;				// Thing Harry should appear to look for when that thing isn't visible
var bool	bLookingForTarget;			// Whether Harry is currently looking around for target

var bool	bReaching;					// Whether Harry has hand stretched out
var Actor	TargetToCatch;				// Thing Harry is in process of catching and putting in hand
var name	PathAfterCatch;				// What path Harry should go fly on after catching target

var name	KickTargetClassName;		// Name of kind of pawn Harry should aim his kick at
var Pawn	KickTarget;					// Current kick target
var float	KickTargetDist;				// How far away the kick target is
var float	fTimeForNextKickUpdate;		// When next to update kick target

var Actor	WatchTarget;				// Current watch target
var float	fTimeForNextWatchUpdate;	// When next to update watch target

const		MAX_REVERSAL_STATS = 20;
var float	fTimesOfLastReversals[20];	// Statistics for how often pitch and yaw reversals are happening
var int		iNextReversalStat;			// Which slot in above array is next to fill
const		fReversalsStatPeriod = 2.5;	// The time window (seconds) for which to compute current reversal rate

//-------------------------------------------------------------------------------------------
// PreBeginPlay(), PostBeginPlay(), and common events
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

	bInvincible = false;
	bAuxBoost = false;
	Deceleration = 0;
	bHasEverBoosted = false;
	bHasEverBraked = false;
	bActioned = false;
	bWasActioning = false;
}

function PostBeginPlay()
{
	local int	iStat;

	// Initialize Harry
	Super.PostBeginPlay();
	SetPhysics(PHYS_Flying);

	// Setup animation layering
	PrimaryAnim = 'Hover';
	SecondaryAnim = '';
	LoopAnim( PrimaryAnim );
	LookForTarget = None;
	bLookingForTarget = false;

	// Load Sounds
	BroomSound = Sound'HPSounds.Quidditch_sfx.Flying_Broom_Loop';
//	MainBroomSound = Sound'HPSounds.Quidditch_sfx.broomlp_nl3';

	HitSounds[0] = Sound'HPSounds.Quidditch_sfx.Q_Collision1';
	HitSounds[1] = Sound'HPSounds.Quidditch_sfx.Q_Collision2';
	HitSounds[2] = Sound'HPSounds.Quidditch_sfx.Q_Collision3';

	// Force rotation rates (ignore settings in editor)
	fRotationRateYaw = 20000;	// Yaw uses this override because roll is tied to RotationRate.Yaw
	RotationRate.Yaw = 50000;
	RotationRate.Roll = 6000;
	RotationRate.Pitch = 24000;

	fBroomSensitivity = 1.0/16384.0;

	fMouseYaw = 0.0;
	fMousePitch = 0.0;
	eYawControlDevice = DEVICE_Button;
	ePitchControlDevice = DEVICE_Button;
	bLastYawNeg = false;
	bLastPitchNeg = false;
	
	bHit = false;
	bHitWall = false;
	bHittingWall = false;
	fLastTimeAvoidedWall = -1.0f;

	bReaching = false;

	fTargetTrackDist = 200.0;
	fTargetTrackHorzOffset = RandRange( -TrackingOffsetRange_Horz, TrackingOffsetRange_Horz );
	fTargetTrackVertOffset = RandRange( -TrackingOffsetRange_Vert, TrackingOffsetRange_Vert );

	// Initialize control stats
	for ( iStat = 0; iStat < MAX_REVERSAL_STATS; ++iStat )
		fTimesOfLastReversals[ iStat ] = -9999.9;	// A time well before the game started
	iNextReversalStat = 0;

	// Add a broom effect.
//	Trail = spawn(class'BroomTrail_02', [SpawnOwner] self);
//	Trail.AttachToOwner('BroomTail');

//	lifePotions = 5;	// *** Test for dying ***
}

event Possess()
{
	// Called when the PlayerPawn Harry is possessed (attached) to a viewport (Player).
	Super.Possess();

	// Make sure the right physics is selected (workaround for bug with
	// LoadGame where physics isn't set correctly)
	Log( "BroomHarry in State "$GetStateName()$"." );

	if ( IsInState( 'BroomDying' ) )
		SetPhysics( PHYS_Falling );
	else
		SetPhysics( PHYS_Flying );
}

event TravelPostAccept()
{
	// Called after all inventory has been accepted from previous level
	super.TravelPostAccept();

	// Get a handle to the Quidditch Armor status
	QuidArmorStatus = StatusItemQArmor( managerStatus.GetStatusItem( class'StatusGroupQGear', class'StatusItemQArmor' ) );
}

event PlayerInput( float DeltaTime )
{
	Super.PlayerInput( DeltaTime );

	// Check the action button
	bActioned = false;
	if ( bBroomAction != 0 )
	{
		if ( !bWasActioning )
		{
			bActioned = true;
			Director.OnActionKeyPressed();
			if ( IsInState( 'PlayerWalking' ) )
			{
				UpdateKickTarget();
				fTimeForNextKickUpdate = Level.TimeSeconds + 1.0;
				if ( KickTarget != None && KickTargetDist < CollisionRadius * 5.0 )
					DoKick( KickTarget );
			}
		}
		bWasActioning = true;
	}
	else
		bWasActioning = false;

	// Check the boost and brake buttons
	if ( bBroomBoost != 0 )
		bHasEverBoosted = true;
	if ( bBroomBrake != 0 )
		bHasEverBraked = true;
}


//-------------------------------------------------------------------------------------------
// CutScene Commands
//-------------------------------------------------------------------------------------------

function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
{
	// A cut-scene is commanding Harry to do something; parse out command and
	// respond accordingly.

	// Commands specific to BroomHarry are handled here; if BroomHarry doesn't
	// know the command, it's passed up to Harry.

	local string	sActualCommand;


	sActualCommand = ParseDelimitedString( Command, " ", 1, false );

	// Dispatch command
	if ( sActualCommand ~= "FlyOnPath" )
	{
		return CutCommand_FlyOnPath( Command, Cue, bFastFlag );
	}
	else
	{
		// Unknown cut-command
		return Super.CutCommand( Command, Cue, bFastFlag );
	}
}

function bool CutCommand_FlyOnPath( string Command, optional string Cue, optional bool bFastFlag )
{
	// Command Harry to fly on a spline path
	// Syntax: FlyOnPath PathName [Start=StartNodeCutName]

	local InterpolationPoint	IP;

	local string			Token;
	local int				i;
	local name				PathName;
	local int				StartPoint;


	// Get name of interpolation path to fly on
	PathName = name( ParseDelimitedString( Command, " ", 2, false ) );

	// Parse remaining optional parameters
	StartPoint = 0;
	i = 2;
	do
	{
		++i;
		Token = ParseDelimitedString( Command, " ", i, false );

		if ( Token != "" )
		{
			if ( Left(Token, Len("Start=")) ~= "Start=" )
			{
				Token = Mid( Token, Len("start=") );

				StartPoint = -1;
				foreach AllActors( class'InterpolationPoint', IP, PathName )
				{
					if ( IP.CutName ~= Token )
					{
						StartPoint = IP.Position;
						break;
					}
				}

				if ( StartPoint == -1 )
				{
					CutErrorString = "No Spline Point on path '"$PathName$"' with CutName '"$Token$"'";
					CutCue( Cue );
					return false;
				}
			}
			else
			{
				ClientMessage( "**** Warning:"$self$":FlyOnPath option '"$Token$"' not recognized.  Ignoring.");
				Log( "**** Warning:"$self$":FlyOnPath option '"$Token$"' not recognized.  Ignoring.");
			}
		}
	} until ( Token == "" );

	if( bFastFlag )
	{
		// Skip to last point on path
		foreach AllActors( class'InterpolationPoint', IP, PathName )
		{
			if ( IP.bEndOfPath )
			{
				SetLocation( IP.Location );
				SetRotation( IP.Rotation );
				break;
			}
		}
		CutCue( Cue );
	}
	else
	{
		// Go fly on it
		CutCommandCue = Cue;
		FlyOnPath( PathName, StartPoint );
	}

	return true;
}

//-------------------------------------------------------------------------------------------
// Operational methods
//-------------------------------------------------------------------------------------------

function SetPrimaryAnimation( name NewPrimaryAnim, optional float Rate, optional float TweenTime,
							  optional float MinRate, optional EAnimType Type, optional name RootBone )
{
	// Sets-up the primary animation to transition to the new animation and
	// makes sure the secondary animation still plays on top of it

	if ( NewPrimaryAnim != PrimaryAnim )
	{
		PrimaryAnim = NewPrimaryAnim;
		if ( PrimaryAnim != '' )
			LoopAnim( PrimaryAnim, /*Rate*/, TweenTime, MinRate /*, Type, RootBone*/ );
		if ( SecondaryAnim != '' )
			LoopAnim( SecondaryAnim, , TweenTime );
	}
}

function SetSecondaryAnimation( name NewSecondaryAnim, optional float Rate, optional float TweenTime,
								optional float MinRate, optional EAnimType Type, optional name RootBone )
{
	// Sets-up the secondary animation to transition to the new animation and
	// makes sure the primary animation still plays under it

	if ( NewSecondaryAnim != SecondaryAnim )
	{
		SecondaryAnim = NewSecondaryAnim;
		if ( PrimaryAnim != '' )
			LoopAnim( PrimaryAnim, , TweenTime );
		if ( SecondaryAnim != '' )
			LoopAnim( SecondaryAnim, /*Rate*/, TweenTime, MinRate /*, Type, RootBone*/ );
	}
}

function DeterminePrimaryAnim()
{
	// Determine which animation is appropriate for current motion
	local float		Speed;

	Speed = VSize(Velocity);

	if ( !bHitWall && !bHit )
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

	Trail.ParentBlend = Min(Speed/200, 1);
}

function SetInvincible( bool bOn )
{
	// If set to True, Harry takes all damage as zero (suffers no health loss).

	bInvincible = bOn;
}

function SetLookForTarget( Actor NewLookForTarget )
{
	// Notes what actor Harry should appear to "look for" when that actor isn't
	// visible.  If None, Harry won't look for anything.

	LookForTarget = NewLookForTarget;
}

function SetTargetTrackDist( float fNewTargetTrackDist )
{
	fTargetTrackDist = fNewTargetTrackDist;
}

function SetReaching( bool bOn )
{
	if ( bReaching != bOn )
	{
		bReaching = bOn;
		if ( bReaching )
			SetSecondaryAnimation( 'GrabAttemptRight', , 1.0 );
		else
			SetSecondaryAnimation( '', , 1.0 );
	}
}

function CatchTarget( Actor Target, optional name PathToFlyAfterCatch )
{
	// Causes Harry to animate a catch and hold sequence, and then put the
	// target in his hand.  The optional PathToFlyAfterCatch is what path
	// Harry should go fly on after catching the target; if None, harry will
	// continue as he's flying.

	TargetToCatch = Target;
	PathAfterCatch = PathToFlyAfterCatch;
	GotoState( 'Catching' );
}

function SetKickTargetClass( name NewKickTargetClassName )
{
	// Notes what kind of actor Harry should aim his kick at.  If None, Harry won't kick at anything.

	if ( WatchTarget == KickTarget )
	{
		StopHeadLook();
		WatchTarget = None;
	}

	KickTargetClassName = NewKickTargetClassName;
}

function UpdateKickTarget()
{
	// Finds nearest kick target and notes it
	local Pawn			Target;
	local Vector		Dir;
	local float			Dist;

	KickTarget = None;

	if ( KickTargetClassName == '' )
		return;

	foreach AllActors( class'Pawn', Target )
	{
		if ( Target.IsA( KickTargetClassName ) )
		{
			Dir = Target.Location - Location;
			Dist = VSize( Dir );
			if ( KickTarget == None || Dist < KickTargetDist )
			{
				KickTarget = Target;
				KickTargetDist = Dist;
			}
		}
	}

	return;
}

function DoKick( Pawn KickTarget )
{
	// Kick or push in direction of kick target.
	local Vector		X,Y,Z;
	local Vector		TargetDir;
	local float			TargetDist;

	local bool			bOnLeft;
	local bool			bKick;

	// What side is target on
	TargetDir = KickTarget.Location - Location;
	TargetDist = VSize( TargetDir );
	GetAxes( Rotation, X, Y, Z );
	bOnLeft = TargetDir dot Y < 0.0f;

	// Decide whether to kick or push
	if ( TargetDir.Z <= -KickTarget.CollisionHeight/3.0f )
		bKick = true;
	else if ( TargetDir.Z >= KickTarget.CollisionHeight/3.0 )
		bKick = false;
	else if ( frand() < 0.5 )		// Random if target is approx. level with Harry
		bKick = true;
	else
		bKick = false;

	// Do animation
	if ( bKick )
	{
		if ( bOnLeft )
			PlayAnim( 'KickLeft', 1.0, 0.2 );
		else
			PlayAnim( 'KickRight', 1.0, 0.2 );
	}
	else		// Push
	{
		if ( bOnLeft )
			PlayAnim( 'PushLeft', 1.0, 0.2 );
		else
			PlayAnim( 'PushRight', 1.0, 0.2 );
	}

	// Impart damage to target
	if ( TargetDist < CollisionRadius * 3.0 )
	{
		KickTarget.TakeDamage( 0, Self, Location, 100*Normal(TargetDir), 'Kicked' );
	}
}

function UpdateWatchTarget()
{
	// Decide what to watch with head
	local bool		bLookAtKickTarget;
	local vector	WatchOffset;
	local float		fWatchWeight;

	if ( KickTarget != None )
	{
		if ( KickTargetDist < 100 )
			bLookAtKickTarget = frand() < 0.75;
		else
			fWatchWeight = 0.75 * ( 400 + 100 - KickTargetDist ) / 400;
		bLookAtKickTarget = frand() < fWatchWeight;
	}
	else
		bLookAtKickTarget = false;

	if ( bLookAtKickTarget )
	{
		if ( WatchTarget != KickTarget )
		{
			WatchTarget = KickTarget;
			WatchOffset = vect( 0, 0, 0 );
			WatchOffset.Z = KickTarget.CollisionHeight * 0.25;
			MakeHeadWatchActor( WatchTarget, WatchOffset );
		}
	}
	else if ( LookForTarget != None )
	{
		if ( WatchTarget != LookForTarget )
		{
			WatchTarget = LookForTarget;
			MakeHeadWatchActor( WatchTarget );
		}
	}
	else
	{
		StopHeadLook();
		WatchTarget = None;
	}
}

//AE:
function PlayFastWhooshSound()
{
	switch( Rand(3) )
	{
//		case 0:	PlaySound(sound'HPSounds.Quidditch_sfx.fast_whoosh_1');	break;
//		case 1:	PlaySound(sound'HPSounds.Quidditch_sfx.fast_whoosh_2');	break;
//		case 2:	PlaySound(sound'HPSounds.Quidditch_sfx.fast_whoosh_4');	break;
	}
}

//AE:
function PlaySlowWhooshSound()
{
	switch( Rand(3) )
	{
//		case 0:	PlaySound(sound'HPSounds.Quidditch_sfx.slow_whoosh_1');	break;
//		case 1:	PlaySound(sound'HPSounds.Quidditch_sfx.slow_whoosh_2');	break;
//		case 2:	PlaySound(sound'HPSounds.Quidditch_sfx.slow_whoosh_3');	break;
	}
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

	//AE: Quiet background, dynamic hiss.			/***/
//	if ( !ModifySound( SOUND_Volume, fTurnFactor, MainBroomSound, SLOT_Interact ) )
//		PlaySound( MainBroomSound, SLOT_Interact, fTurnFactor, true, , 1.0 );

	// Change the broom sound parameters
//	if ( !ModifySound( SOUND_Volume, fVolume, BroomSound, SLOT_Misc ) )
//		PlaySound( BroomSound, SLOT_Misc, fVolume, true, , fPitch );	// Wasn't already playing; start it now
//	else
//		ModifySound( SOUND_Pitch, fPitch, BroomSound, SLOT_Misc );
}

function InterpolationPoint FindPointOnPath( name Path, optional int PointToFind )
{
	// Finds the point on the named path that has a Position Id matching the
	// specified PointToFind.  If PointToFind is not specified, the lowest-
	// numbered point is sought.  Returns a reference to the found point, or None
	// if not found.

	local InterpolationPoint	IP;
	local InterpolationPoint	FoundIP;

	FoundIP = None;
	foreach AllActors( class'InterpolationPoint', IP, Path )
	{
		if ( IP.Position == PointToFind )
		{
			FoundIP = IP;
			break;
		}
		else if ( PointToFind == 0 && ( FoundIP == None || IP.Position < FoundIP.Position ) )
			FoundIP = IP;
	}

	return FoundIP;
}

function FlyOnPath( name CutScenePath, optional int StartPoint )
{
	local InterpolationPoint	i;

	if ( CutScenePath != '' )
	{
		// Put Harry at start point on its path
		i = FindPointOnPath( CutScenePath, StartPoint );

		if ( i != None )
		{
			SetLocation( i.Location );
			SetRotation( i.Rotation );
			SetCollision( [NewColActors] true, [NewBlockActors] true, [NewBlockPlayers] true );
			bCollideWorld = true;
			bInterpolating = true;
			SetPhysics( PHYS_None );

			IM = Spawn( class'InterpolationManager', self );
			IM.Init( i.Next, 1.0, false );
		}

		if ( IM == None )
		{
			Log( "Harry couldn't find path "$CutScenePath );
		}
		else
		{
			GotoState( 'FlyingOnPath' );
		}
	}
}

function StopFlyingOnPath()
{
	local InterpolationManager	IM_ToStop;

	if ( IM != None )
	{
		IM_ToStop = IM;
		IM = None;		// Tells FinishInterpolation event that path ended early
		IM_ToStop.FinishedInterpolation( None );
	}
	bCollideWorld = true;
	SetCollision( [NewColActors] true, [NewBlockActors] true, [NewBlockPlayers] true );

	if ( IsInState( 'FlyingOnPath' ) )
		GotoState( 'PlayerWalking' );
}

function bool GetOnPath( name Path, optional int StartPoint )
{
	// Create and fly on a temporary path that connects with the named path.
	// If StartPoint is omitted (or 0), the temporary path connects to
	// the second closest point on the destination path; otherwise, the
	// connection is made at the specified StartPoint on the destination path.
	local float					fDistance;
	local InterpolationPoint	i;
	local vector				X,Y,Z;

	local float					fClosestDistance;
	local InterpolationPoint	ClosestPoint;
	local int					iClosestPoint;

	local float					fTransDistance;
	local InterpolationPoint	TransPoint;


	if ( Path == '' )
		return false;

	if ( StartPoint != 0 )
	{
		// Find the specified point on path
		foreach AllActors( class'InterpolationPoint', i, Path )
		{
			if ( i.Position == StartPoint )
			{
				TransPoint = i;
				fTransDistance = fDistance;
				iTransPoint = i.Position;
				break;
			}
		}
	}
	else
	{
		// Find second closest point on path (we use the second closest
		// point to build a return path to, because if we used the closest
		// point it could be so close that the return path would be too
		// abrupt and tight)
		ClosestPoint = None;
		fClosestDistance = 999999.0;
		iClosestPoint = 0;

		TransPoint = None;
		fTransDistance = 999999.0;
		foreach AllActors( class'InterpolationPoint', i, Path )
		{
			fDistance = VSize( Location - i.Location );
			if ( fDistance < fClosestDistance )
			{
				fTransDistance = fClosestDistance;
				TransPoint  = ClosestPoint;
				iTransPoint = iClosestPoint;

				ClosestPoint = i;
				fClosestDistance = fDistance;
				iClosestPoint = i.Position;
			}
			else if ( fDistance < fTransDistance )
			{
				TransPoint = i;
				fTransDistance = fDistance;
				iTransPoint = i.Position;
			}
		}

		if ( TransPoint == None )
		{
			fTransDistance = fClosestDistance;
			TransPoint  = ClosestPoint;
			iTransPoint = iClosestPoint;
		}
	}

	if ( TransPoint == None )
	{
		Log( "BroomHarry could not find an interpolation point on path to go to" );
		return false;
	}

	// Create a transition path to desired path...
	// Set-up interpolation point at end of return path
	if ( TransPath[1] == None )
	{
		TransPath[1] = Spawn( class'DynamicInterpolationPoint' );
		TransPath[1].Tag = TransPath[1].Name;
		TransPath[1].Position = 1;
		TransPath[1].bEndOfPath = true;
	}
	TransPath[1].SetLocation( TransPoint.Location );
	TransPath[1].SetRotation( TransPoint.Rotation );
//	TransPath[1].DesiredSpeed = TransPoint.DesiredSpeed;
	TransPath[1].DesiredSpeed = 900;
	TransPath[1].StartControlPoint = TransPoint.StartControlPoint;
	TransPath[1].EndControlPoint = TransPoint.EndControlPoint;

	// Set-up interpolation point at beginning of return path
	if ( TransPath[0] == None )
	{
		TransPath[0] = Spawn( class'DynamicInterpolationPoint', , TransPath[1].Tag );
		TransPath[0].Position = 0;
		TransPath[0].bEndOfPath = false;
		TransPath[0].Next = TransPath[1];
		TransPath[0].Prev = TransPath[1];
		TransPath[1].Next = TransPath[0];
		TransPath[1].Prev = TransPath[0];
	}

	TransPath[0].SetLocation( Location );
	TransPath[0].SetRotation( Rotation );
	TransPath[0].DesiredSpeed = VSize( Velocity );

	GetAxes( Rotation, X, Y, Z );
	TransPath[0].StartControlPoint =  1000.0 * X;
	TransPath[0].EndControlPoint   = -1000.0 * X;

	// Start flying on transition path;
	SetCollision( [NewColActors] true, [NewBlockActors] true, [NewBlockPlayers] true );
	bCollideWorld = true;
	bInterpolating = true;
	SetPhysics( PHYS_None );

	IM = Spawn( class'InterpolationManager', self );
	IM.Init( TransPath[1], 1.0, false );

	return true;
}

function TakeDamage( int Damage, Pawn InstigatedBy, Vector HitLocation, 
					 Vector Momentum, name DamageType )
{
	// Harry got hit by something bad
	if ( IsInState( 'PlayerWalking' ) )
	{
		StopHeadLook();
		WatchTarget = None;
		PlayAnim( 'Bump', , 0.2 );
		bHit = true;

		PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );

		if ( !bInvincible )
		{
			// Adjust for having Quidditch Armor
//			if ( QuidArmorStatus.GetCount() > 0 )
				ClientMessage( "BroomHarry Damage="$Damage );
				Damage *= ArmorDamageScale;
				ClientMessage( "BroomHarry EffectiveDamage="$Damage );

			AddHealth( -Damage );
			if( GetHealthCount() <= 0.0 )
				KillHarry(true);
		}
	}
}

function KillHarry( bool bImmediateDeath )
{
	ClientMessage( "I can't go on!!!!" );
	GotoState( 'BroomDying' );
}

function PlayinAir()
{
	// Override to keep walking harry logic from playing fall animation
}

function PlayWaiting()
{
	// Override to keep walking harry logic from playing idle animations
}

function NoteAnotherReversal()
{
	// Updates stats on number of pitch and yaw reversals that have happened
	// recently.  Time of current reversal is noted and only the most recent
	// reversals are remembered.

	// Note current time for this reversal
	fTimesOfLastReversals[ iNextReversalStat ] = Level.TimeSeconds;

	// Move to next slot in stats array
	++iNextReversalStat;
	if ( iNextReversalStat >= MAX_REVERSAL_STATS )
		iNextReversalStat = 0;
}

function float GetReversalsPerSecond()
{
	// Computes and returns the current rate of control reversals (changes in
	// pitch or yaw directions) in number of reversals per second.  The rate is
	// the average over the period defined by the fReversalsStatPeriod constant.

	local int	iStat;
	local int	iReversals;
	local float	fReversalRate;

	// Count back a number of reversals until the delta time exceeds the stats
	// sample period
	iReversals = 0;
	iStat = iNextReversalStat;
	do
	{
		if ( iStat == 0 )
			iStat = MAX_REVERSAL_STATS;
		--iStat;

		if ( Level.TimeSeconds - fTimesOfLastReversals[ iStat ] > fReversalsStatPeriod )
			break;

		++iReversals;
	} until ( iReversals >= MAX_REVERSAL_STATS );

	// Compute average per second
	fReversalRate = iReversals / fReversalsStatPeriod;

	return fReversalRate;
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
		local BaseCam	Camera;
		local Seeker	WatchTarget;

		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );

		super.BeginState();
		SetPhysics(PHYS_Flying);

		// If no director present, find camera and set it into Quidditch mode to follow Harry
		// (this condition happens when levels are under construction)
		if ( Director == None )
		{
			foreach AllActors( class'BaseCam', Camera )
			{
				Camera.SetCameraMode( CM_Quidditch );
				break;
			}
		}

		fTimeForNextKickUpdate = Level.TimeSeconds;
		fTimeForNextWatchUpdate = Level.TimeSeconds;
	}

	function EndState()
	{
		ClientMessage( "BroomHarry: Exited "$GetStateName()$" State" );
		Log( "BroomHarry: Exited "$GetStateName()$" State" );

		StopHeadLook();
		WatchTarget = None;

		Super.EndState();
	}

	// Overrides regular Harry's tick behavior to focus on flying movement
	event PlayerTick( float DeltaTime )
	{
		// If target, track it; otherwise fly freely
		if ( LookForTarget != None )
			PlayerTrack( DeltaTime );
		else
			PlayerMove( DeltaTime );

		// If time to update the kick target do it
		if ( Level.TimeSeconds > fTimeForNextKickUpdate )
		{
			UpdateKickTarget();
			fTimeForNextKickUpdate = Level.TimeSeconds + 1.0;
		}

		// If time to update the watch target do it
		if ( Level.TimeSeconds > fTimeForNextWatchUpdate )
		{
			UpdateWatchTarget();
			fTimeForNextWatchUpdate = Level.TimeSeconds + RandRange( 1.0, 1.2 );
		}
	}

	// Harry automatically follows a target; controls limited to horizontal
	// and vertical lateral offsets
	function PlayerTrack( float DeltaTime )
	{
		local vector	X,Y,Z;						// Harry's orientation vectors
		local vector	TargetX, TargetY, TargetZ;	// Target's orientation vectors
		local vector	TargetTrackPoint;			// Point behind target Harry should close in on

		local vector	TargetDir;
		local float		TargetDist;
		local float		TargetSpeed;

		local float		fDeltaPitch;

		const	SlowdownRadius = 50;	// How close to target should pursuer begin slowing to match speed
		const	MaxSpeed = 1200;		// How fast can pursuer go

		// Update tracking offsets from inputs
		fTargetTrackHorzOffset += (bBroomYawRight-bBroomYawLeft) * 350.0 * DeltaTime;
		fTargetTrackHorzOffset += aBroomYaw * fBroomSensitivity * TrackingOffsetRange_Horz;
		if ( fTargetTrackHorzOffset > TrackingOffsetRange_Horz )
			fTargetTrackHorzOffset = TrackingOffsetRange_Horz;
		else if ( fTargetTrackHorzOffset < -TrackingOffsetRange_Horz )
			fTargetTrackHorzOffset = -TrackingOffsetRange_Horz;

		fDeltaPitch = (bBroomPitchUp-bBroomPitchDown) * 350.0 * DeltaTime;
		fDeltaPitch -= aBroomPitch * fBroomSensitivity * TrackingOffsetRange_Vert;
		if ( bInvertBroomPitch ) 
			fDeltaPitch = -fDeltaPitch;
		fTargetTrackVertOffset += fDeltaPitch;
		if ( fTargetTrackVertOffset > TrackingOffsetRange_Vert )
			fTargetTrackVertOffset = TrackingOffsetRange_Vert;
		else if ( fTargetTrackVertOffset < -TrackingOffsetRange_Vert )
			fTargetTrackVertOffset = -TrackingOffsetRange_Vert;

		// Determine track points
		GetAxes( LookForTarget.Rotation, TargetX, TargetY, TargetZ );	// Get Target's orientation vectors
		TargetTrackPoint = LookForTarget.Location
						 + TargetY * fTargetTrackHorzOffset
						 + TargetZ * (fTargetTrackVertOffset - CollisionHeight);	// Adjust for grab height

		// Update Rotation to point Harry at target's track point
		TargetDir = TargetTrackPoint - Location;
		DesiredRotation = Rotator( TargetDir );

		// Update camera offsets to reflect tracking offsets
//		Cam.SetYOffset( -fTargetTrackHorzOffset );
//		Cam.SetZOffset( -fTargetTrackVertOffset + CollisionHeight );

		// Update acceleration and speed
		GetAxes( Rotation, X, Y, Z );	// Get Harry's orientation vectors
		AccelRate = 5000.0;
		Acceleration = AccelRate * X;	// Make speed changes have instant effect
		DesiredSpeed = 1.0;				// Scales AirSpeed in physFlying

		TargetDist = VSize( TargetDir ) - fTargetTrackDist;
		if ( TargetDist > SlowdownRadius )
			AirSpeed = MaxSpeed;
		else
		{
			TargetSpeed = VSize( LookForTarget.Velocity );
			AirSpeed = TargetDist / SlowdownRadius * (MaxSpeed - TargetSpeed) + TargetSpeed;
			if ( AirSpeed < 0 )
				AirSpeed = 0;
		}

//		Log( "*** *** *** *** BHarry: Track Dist = "$fTargetTrackDist$" Dist Error = "$TargetDist$" Air Speed = "$AirSpeed$" Vel = "$VSize( Velocity ) );

		// Determine which animation is appropriate for current motion
		DeterminePrimaryAnim();

		// Layer-on a Look animation if Harry should be looking for something
		// (and that something isn't currently visible)
		if ( LookForTarget != None && (LookForTarget.bHidden || !CanSee( LookForTarget )) )
		{
			if ( !bLookingForTarget )
			{
				bLookingForTarget = true;
				SetTimer( frand() * 3.0 + 1.0, false );
			}
		}
		else
		{
			// Target is in view; stop looking for it
			if ( bLookingForTarget )
			{
				bLookingForTarget = false;
				SetTimer( 0.0, false );
			}
		}

		// Update the broom sound effects
		UpdateBroomSound();
	}

	// Overrides regular Harry's movement behavior to remap controls for
	// flying (using forward controls to control pitch)
	function PlayerMove( float DeltaTime )
	{
		local vector	X,Y,Z, NewAccel;
		local float		DecelRate, HiDecelRate;

		HiDecelRate = 0.2;
		DecelRate = 2.5;

		// Figure out which inputs are controlling pitch
		//
		if ( abs( bBroomPitchUp ) > 0.0005 || abs( bBroomPitchDown ) > 0.0005 )
		{
			if ( ePitchControlDevice == DEVICE_Mouse )
			{
				// Changing pitch control from mouse
				fMousePitch = 0.0;
			}
			ePitchControlDevice = DEVICE_Button;
		}
		else if ( abs( aJoyBroomPitch ) > 0.0005 )
		{
			if ( ePitchControlDevice == DEVICE_Mouse )
			{
				// Changing pitch control from mouse
				fMousePitch = 0.0;
			}
			ePitchControlDevice = DEVICE_Joystick;
		}
		else if ( bAllowBroomMouse && abs( aBroomPitch ) > 0.0005 )
		{
			ePitchControlDevice = DEVICE_Mouse;
		}

		// Figure out which inputs are controlling yaw
		//
		if ( abs( bBroomYawLeft ) > 0.0005 || abs( bBroomYawRight ) > 0.0005 )
		{
			if ( eYawControlDevice == DEVICE_Mouse )
			{
				// Changing yaw control from mouse
				fMouseYaw = 0.0;
			}
			eYawControlDevice = DEVICE_Button;
		}
		else if ( abs( aJoyBroomYaw ) > 0.0005 )
		{
			if ( eYawControlDevice == DEVICE_Mouse )
			{
				// Changing yaw control from mouse
				fMouseYaw = 0.0;
			}
			eYawControlDevice = DEVICE_Joystick;
		}
		else if ( bAllowBroomMouse && abs( aBroomYaw ) > 0.0005 )
		{
			eYawControlDevice = DEVICE_Mouse;
		}

		// Interpret pitch controls for flying
		//
		switch (ePitchControlDevice)
		{
		case DEVICE_Mouse:
			if ( bInvertBroomPitch )
				fMousePitch += aBroomPitch * fBroomSensitivity;
			else
				fMousePitch -= aBroomPitch * fBroomSensitivity;

			// Limit to keep logical mouse center point from drifting to far
			// from pad physical center
			//
			if ( fMousePitch > 1.5 )
				fMousePitch = 1.5;
			else if ( fMousePitch < -1.5 )
				fMousePitch = -1.5;

			break;
		
		case DEVICE_Joystick:
			if ( bInvertBroomPitch )
				fPitchControl = aJoyBroomPitch * fBroomSensitivity;
			else
				fPitchControl = -aJoyBroomPitch * fBroomSensitivity;

			break;

		
		case DEVICE_Button:
			fPitchControl = 1.0*bBroomPitchUp - 1.0*bBroomPitchDown;
			if ( bInvertBroomPitch )
				fPitchControl = -fPitchControl;
		
			break;
		
		default:
			fPitchControl = 0.0;
			
			break;
		}

		// Interpret yaw controls for flying
		switch (eYawControlDevice)
		{
		case DEVICE_Mouse:
			fMouseYaw += aBroomYaw * fBroomSensitivity;

			if ( abs( aBroomYaw ) > 0.0005 )
			{
//				ClientMessage( "Yaw "$aBroomYaw$" Sum "$fMouseYaw );
//				Log( "Yaw "$aBroomYaw$" Sum "$fMouseYaw );
			}


			// Limit to keep logical mouse center point from drifting to far
			// from pad physical center
			if ( fMouseYaw > 1.5 )
				fMouseYaw = 1.5;
			else if ( fMouseYaw < -1.5 )
				fMouseYaw = -1.5;

			// Adjust for deadband
			if ( fMouseYaw > 0.5 )
				fYawControl = fMouseYaw - 0.3;
			else if ( fMouseYaw < -0.5 )
				fYawControl = fMouseYaw + 0.3;
			else
				fYawControl = 0.0;

			break;

		case DEVICE_Joystick:
			fYawControl = aJoyBroomYaw * fBroomSensitivity;
			break;
		
		case DEVICE_Button:
			fYawControl = 1.0 * bBroomYawRight - 1.0 * bBroomYawLeft;
			break;

		default:
			fYawControl = 0.0;
			break;
		}

		// Update rotation.
		UpdateRotation(DeltaTime, 1);
		GetAxes(Rotation,X,Y,Z);

		// Update acceleration.
		Acceleration = 200000.0 * X;

		// Determine airspeed according to brake setting
		if ( (bBroomBoost != 0 || bAuxBoost) && bBroomBrake == 0 )
		{
			AirSpeed = AirSpeedBoost;
		}
		else
		{
			if ( abs( fYawControl ) > 0.2 || bBroomBrake != 0 )
			{
				if (Deceleration < AirSpeedNormal / 2)
				{
					Deceleration += ((AirSpeedNormal / 2) / DecelRate) * deltatime;
				}
			}
			else
			{
				if (Deceleration > 0)
				{
					if ( Deceleration > (0.4 * AirSpeedNormal) )
						Deceleration -= ((AirSpeedNormal / 2) / HiDecelRate) * deltatime;
					else
						Deceleration -= ((AirSpeedNormal / 2) / DecelRate) * deltatime;

					if (Deceleration < 0)
					{
						Deceleration = 0;
					}
				}
			}
			AirSpeed = AirSpeedNormal - Deceleration;
		}

		// Compute a point passed where Harry will be if he doesn't get blocked
		// (used in HitWall event filter)
		Destination = (AirSpeed * DeltaTime + 0.1) * X + Location;

		// Determine which animation is appropriate for current motion
		DeterminePrimaryAnim();

		// Layer-on a Look animation if Harry should be looking for something
		// (and that something isn't currently visible)
		if ( LookForTarget != None && (LookForTarget.bHidden || !CanSee( LookForTarget )) )
		{
			if ( !bLookingForTarget )
			{
				bLookingForTarget = true;
				SetTimer( frand() * 3.0 + 1.0, false );
			}
		}
		else
		{
			// Target is in view; stop looking for it
			if ( bLookingForTarget )
			{
				bLookingForTarget = false;
				SetTimer( 0.0, false );
			}
		}

		// Update the broom sound effects
		UpdateBroomSound();
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

		NewRotation = Rotation;
		fPitchLimitHi = PitchLimitUp * (0x4000/90.0);
		fPitchLimitLo = 0x00010000 - (PitchLimitDown * (0x4000/90.0));

		// Modify pitch using current input source
		switch (ePitchControlDevice)
		{
		case DEVICE_Mouse:
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

			break;
		
		case DEVICE_Joystick:
			// See if pitch control has reversed
			//
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

			// Update pitch
			NewRotation.Pitch += RotationRate.Pitch * DeltaTime * fPitchControl;
			NewRotation.Pitch = NewRotation.Pitch & 0x0000ffff;

			// Limit pitch
			If ( (NewRotation.Pitch > fPitchLimitHi) && (NewRotation.Pitch < fPitchLimitLo) )
			{
				If (fPitchControl > 0) 
					NewRotation.Pitch = fPitchLimitHi;
				else
					NewRotation.Pitch = fPitchLimitLo;
			}
			break;

		case DEVICE_Button:
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
			NewRotation.Pitch = NewRotation.Pitch & 0x0000ffff;

			// Limit pitch
			If ( (NewRotation.Pitch > fPitchLimitHi) && (NewRotation.Pitch < fPitchLimitLo) )
			{
				If (fPitchControl > 0) 
					NewRotation.Pitch = fPitchLimitHi;
				else
					NewRotation.Pitch = fPitchLimitLo;
			}

			break;
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

		ViewRotation.Yaw += yawVal;

//		Log( "YawControl, YawVal: "$fYawControl$", "$YawVal );

		ViewShake(deltaTime);
			
		NewRotation.Yaw = ViewRotation.Yaw;

		// Commit new rotation
		setRotation(NewRotation);
		DesiredRotation = Rotation; //Make physicsRotation leave rotation alone
//		ClientMessage("Rotation="$Rotation);
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

		// Ignore hits with flat ceilings (usually an invisible BlockAll on a sky)
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
			PlaySound( HitSounds[ Rand( NUM_HIT_SOUNDS ) ], SLOT_Interact, fVolume );

			// Take damage, but don't let Harry go into Hit state (thus the skipping over parent class)
			if ( WallDamage > 0 )
			{
				EffectiveDamage = WallDamage * fSpeed / AirSpeedNormal;

				if ( !bInvincible && EffectiveDamage > 0 )
				{
					PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );

					AddHealth( -EffectiveDamage );
					if( GetHealthCount() <= 0.0 )
						KillHarry(true);
				}
			}

			// Play bump animation
			StopHeadLook();
			WatchTarget = None;
			PlayAnim( 'Bump', , 0.1 );

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

//		Log( "-------------" );
//		Log( "Time, fLastTimeAvoidedWall: "$Level.TimeSeconds$", "$fLastTimeAvoidedWall );
//		Log( "Normal: "$HitNormal );
//		Log( "bTurnToRight: "$bTurnToRight );
//		Log( "FlightDir, Rotation, WallFaceRot: "$FlightDir$", "$Rotation.Yaw$", "$WallFaceRot );
//		Log( "DeltaYaw: "$WallAvoidanceYaw );

		bHittingWall = true;	// Note that Harry's still in contact with wall
	}

	function Bump( Actor Other )
	{
		// Harry hit something, he stumbles, and imparts damage to other
		// actor, but only if it's the kick target pawn.
		local Pawn	Target;

		Target = Pawn( Other );
		if ( !bHit && KickTarget != None && Target == KickTarget )
		{
			PlayAnim( 'React' );

			PlaySound( HitSounds[ Rand( NUM_HIT_SOUNDS ) ], SLOT_Interact, 0.7, , 1000.0 );	// Radius makes sure player can be heard near by

			Target.TakeDamage( 0, Self, Location, 100*Normal(Velocity), 'Collided' );

			Velocity = vect(0,0,1);
			bHit = true;
		}
	}

	event Timer()
	{
		// Time to do another look-around for target
		PlayAnim( 'Look', , 1.0 );
		SetTimer( frand() * 4.0 + 1.5, false );		// Play it again a little later
	}

	function AnimEnd()
	{
		// Restarts the layered primary and secondary animations in case any part
		// of them were overridden by the non-looping animation that just finished.

		if ( PrimaryAnim != '' )
			LoopAnim( PrimaryAnim, , 1.0 );
		if ( SecondaryAnim != '' )
			LoopAnim( SecondaryAnim, , 1.0 );

		bHitWall = false;	// Note that Harry is done reacting to wall hit
		bHit = false;		// Note that Harry is done reacting to actor bump
	}
}

state stateCutIdle
{
	ignores AltFire, Mount;

	function BeginState()
	{
		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );

		AirSpeed = 0;
	}

	// Overrides regular Harry's tick behavior to react to scripted path movement
	event PlayerTick( float DeltaTime )
	{
		Super.PlayerTick( DeltaTime );
		DeterminePrimaryAnim();
		UpdateBroomSound();
	}

	function AnimEnd()
	{
		// Restarts the layered primary and secondary animations in case any part
		// of them were overridden by the non-looping animation that just finished.

		if ( PrimaryAnim != '' )
			LoopAnim( PrimaryAnim, , 1.0 );
		if ( SecondaryAnim != '' )
			LoopAnim( SecondaryAnim, , 1.0 );
	}
}

state FlyingOnPath
{
	ignores AltFire, Mount;

	function BeginState()
	{
		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );
	}

	// Overrides regular Harry's tick behavior to react to path movement
	event PlayerTick( float DeltaTime )
	{
		Super.PlayerTick( DeltaTime );
		ViewRotation = Rotation;
		DeterminePrimaryAnim();
		UpdateBroomSound();
	}

	function AnimEnd()
	{
		// Restarts the layered primary and secondary animations in case any part
		// of them were overridden by the non-looping animation that just finished.

		if ( PrimaryAnim != '' )
			LoopAnim( PrimaryAnim, , 1.0 );
		if ( SecondaryAnim != '' )
			LoopAnim( SecondaryAnim, , 1.0 );
	}

	event FinishedInterpolation( InterpolationPoint Other )
	{
		// This event happens when Harry reaches the end-point on
		// the spline path.
		if ( IM != None )
		{
			IM = None;
			if ( CutCommandCue != "" )
			{
				CutCue( CutCommandCue );
				CutCommandCue = "";
				GotoState( 'stateCutIdle' );
			}
		}
	}
}

state Pursue
{
	ignores AltFire, Mount;

	function BeginState()
	{
		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );
	}

	event PlayerTick( float DeltaTime )
	{
		local vector	TargetDir;
		local vector	X,Y,Z;

		Super.PlayerTick( DeltaTime );

		if ( LookForTarget == None || LookForTarget.bHidden )
			GotoState( 'PlayerWalking' );

		// Update Rotation to point at target
		TargetDir = LookForTarget.Location - Location;
		DesiredRotation = Rotator( TargetDir );

		// Update acceleration and speed
		GetAxes( Rotation, X, Y, Z );
		Acceleration = 200000.0 * X;
		if ( VSize( TargetDir ) < 150.0 )
			AirSpeed = VSize( LookForTarget.Velocity ) * 1.0;
		else if ( VSize( TargetDir ) < 300.0 )
			AirSpeed = VSize( LookForTarget.Velocity ) * 1.25;
		else
			AirSpeed = VSize( LookForTarget.Velocity ) * 1.9;
		DesiredSpeed = AirSpeed;

		// Update animation and sound
		ViewRotation = Rotation;
		DeterminePrimaryAnim();
		UpdateBroomSound();
	}

	function EndState()
	{
		ClientMessage( "BroomHarry: End Pursue" );
		Log( "BroomHarry: End Pursue" );
	}
}

state Hit
{
	ignores AltFire, Mount;

	function BeginState()
	{
		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );
	}

	event PlayerTick( float DeltaTime )
	{
		Super.PlayerTick( DeltaTime );
		UpdateBroomSound();
	}

Begin:
	PlayAnim( 'Bump' );
	FinishAnim();
	bHitWall = false;	// Note that Harry is done reacting to wall hit, if that's what he hit
	GotoState( 'PlayerWalking' );
}

state BroomDying
{
	ignores AltFire, Mount;

	function BeginState()
	{
		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );
	}

	event PlayerTick( float DeltaTime )
	{
		Super.PlayerTick( DeltaTime );
		UpdateBroomSound();
	}

	function Landed( vector HitNormal )
	{
		// Fell to ground, now dead

//		PlaySound( Sound'HPSounds.Quidditch_sfx.Q_Harry_Crash', SLOT_Interact );
		Director.OnPlayersDeath();
		SetTimer( 0.0, false );
	}

	function Timer()
	{
		// Never reached ground, dead anyway

		Director.OnPlayersDeath();
	}

	function AnimEnd()
	{
		// Ignore
	}

Begin:
	PlayAnim( 'Fall' );
	FinishAnim();
	Director.OnPlayerDying();
	LoopAnim( 'Hang' );
//	Cam.GotoState( 'TopDownState' );	// *** Temp disabled: waiting on appropriate camera mode
	SetPhysics( PHYS_Falling );
	SetTimer( 10.0, false );		// Watchdog timer in case Harry never reaches ground

Loop:
	Sleep( 0.1 );

	goto 'Loop';
}

state Catching
{
	ignores AltFire, Mount;

	function BeginState()
	{
		ClientMessage( "BroomHarry: Entered "$GetStateName()$" State" );
		Log( "BroomHarry: Entered "$GetStateName()$" State" );
	}

	event PlayerTick( float DeltaTime )
	{
		Super.PlayerTick( DeltaTime );
		ViewRotation = Rotation;
		UpdateBroomSound();

//		Log( "Location Z: "$Location.z );
	}

	function EndState()
	{
		ClientMessage( "BroomHarry: End GetOnPath" );
		Log( "BroomHarry: End GetOnPath" );
//		StopFlyingOnPath();
	}

	event FinishedInterpolation( InterpolationPoint Other )
	{
		// This event happens when Harry reaches the end-point on
		// the transition path.  Start flying on intended path.
		if ( IM != None )
		{
			IM = None;
			IPSpeed = 900;
			FlyOnPath( PathAfterCatch, iTransPoint );
		}
	}

Begin:
	// Start catch animation
	if ( TargetToCatch == LookForTarget )
		SetLookForTarget( None );
	PlayAnim( 'Catch', , 0.1 );

	Sleep( 0.4 );			// Dead reckon when Catch animation has Harry's hand closing in around target

	// Attach target to Harry's hand
	TargetToCatch.SetPhysics( PHYS_Trailer );
	TargetToCatch.SetOwner( Self );
	TargetToCatch.AttachToOwner( 'RightHand' );
	TargetToCatch.bTrailerPrePivot = true;
	TargetToCatch.PrePivot = vect( 3, -3, 0 );		// Offset snitch so it doesn't engulf hand

	// Wait for catch animation to end, then hold with target in hand
	FinishAnim();
	SetSecondaryAnimation( 'Hold', , 0.1 );

	// Reverse camera
	Cam.SetCameraMode( CM_Standard );	// *** Temp: waiting on appropriate camera mode
	//StandardTarget.TargetOffset = vect(-100, 10 ,50);

	// WIll still be in lockaroundharrymode
//		Cam.TargetRot = rot(5000, 0, 0);

	// Transition Harry to desired path now
	if ( PathAfterCatch != '' )
	{
		if ( !GetOnPath( PathAfterCatch ) )
			Log( "BroomHarry failed to get on path "$PathAfterCatch$" after catch" );
	}

Loop:
	Sleep( 0.1 );

	goto 'Loop';
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skHarryQuidMesh'
//	ShadowClass=class'BroomShadow'
	bAlignBottom=false
	MaxMountHeight=0.0	// Harry: thou shalt not auto-mount while flying
	AirSpeedNormal=400
	AirSpeedBoost=800
	PitchLimitUp=60
	PitchLimitDown=60
	RotationRate=(Yaw=50000)
	RotationRate=(Roll=6000)
	RotationRate=(Pitch=24000)

	TrackingOffsetRange_Horz=100
	TrackingOffsetRange_Vert=75

	WallDamage=1
	ArmorDamageScale=0.6
}
