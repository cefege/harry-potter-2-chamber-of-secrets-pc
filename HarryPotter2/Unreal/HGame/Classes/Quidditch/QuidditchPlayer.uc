//=============================================================================
// QuidditchPlayer  -- Another player in Quidditch
//=============================================================================
class QuidditchPlayer extends HChar;

var Director		Director;				// Object in charge of the rules of the current mini-game

var InterpolationManager		IM;
var(Quidditch) name				Path;			// Interpolation path to follow
var(Quidditch) name				Path_Intro;		// Interpolation path to follow during game intro
var name						PathToFly;		// Which interpolation path to follow right now

var DynamicInterpolationPoint	ReturnPath[2];	// Path to follow to get back on main path
var int							iReturnPoint;	// Position Id of point on main path that player is returning to

var Actor			LookForTarget;			// Thing player should appear to look for when that thing isn't visible
var bool			bCaughtTarget;			// Player is currently holding the target he was looking for

var float			fTimeForNextOffset;			// When next to change the target track offset
var float			fTargetTrackDist;			// How far back from target should player try to maintain position
var float			fTargetTrackHorzOffset;		// How far left(-) or right(+) of center line should player track the target
var float			fTargetTrackVertOffset;		// How far below(-) or above(+) center line should player track the target
var	float			TrackingOffsetRange_Horz;	// Maximum horizontal offset player can track target with
var	float			TrackingOffsetRange_Vert;	// Maximum vertical offset player can track target with

var name			KickTargetClassName;	// Name of kind of pawn player should aim his kick at
var Pawn			KickTarget;				// Current kick target
var float			KickTargetDist;			// How far away the kick target is
var float			fTimeForNextKickUpdate;	// When next to update kick target
var float			fTimeForNextKick;		// When next to try a kick

var Actor			WatchTarget;			// Current watch target
var float			fTimeForNextWatchUpdate;// When next to update watch target

var bool			bStunned;				// Whether player has been stunned by enough kicks
var float			fTimeToFallAway;		// When to fall out of pursuit after being stunned

var float				fHealth;				// Float version of health (used instead of standard health)
var(Quidditch) float	HealthRecoveryRate;		// Recovery speed, health per second
var(Quidditch) int		Damage;					// How much damage player imparts on impact

const				NUM_HIT_SOUNDS = 3;
var Sound			HitSounds[3];			// All the different sounds of player hitting something
var bool			bHit;					// Player is currently reacting to a hit

var bool			bCapturedByCutScene;	// Whether Cut-Scene has control of player right now


enum HouseAffiliation	// Must be in same order as in QuidditchDirector.uc
{
	HA_Gryffindor,		// House 0
	HA_Ravenclaw,		// House 1
	HA_Hufflepuff,		// House 2
	HA_Slytherin,		// House 3

	HA_DependsOnMatch,	// Used to auto select the house based on team and match settings
};

const HA_Neutral  = 0;	// Used when the specific house isn't important

enum TeamAffiliation	// What team a player is on; must be in same order as in QuidditchDirector.uc
{
	TA_Gryffindor,
	TA_Opponent,
	TA_Neutral
};

enum Sex				// What sex the player is
{
	SX_Male,
	SX_Female,
	SX_Neutral
};

const QP_NUM_MULTISKINS = 8;				// If this changes, change MultiSkins[] in struct DisplayInfo too

struct DisplayInfo
{
	var() Sex		Sex;					// The sex of the player for this variation of player
	var() Mesh		Mesh;					// Mesh to use for this variation of player
	var() Texture	MultiSkins[8];			// The MultiSkin textures to use for this variation of player
};

var(Quidditch) DisplayInfo		HouseDisplayInfo[4];	// House-specific display variations for this player
var(Quidditch) TeamAffiliation	Team;					// Which team is the player affiliated with
var(Quidditch) HouseAffiliation	HouseToDisplayAs;		// The desired house for the player to be affiliated with

var HouseAffiliation	eHouse;				// The actual house the player is affiliated with this match
var Sex					eSex;				// The actual sex the player is this match

//-------------------------------------------------------------------------------------------
// PreBeginPlay(), PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	Super.PreBeginPlay();

	LookForTarget = None;
	bCaughtTarget = false;
	iReturnPoint = 0;
	bCapturedByCutScene = false;
	bStunned = false;
	fHealth = 100.0;
}

function PostBeginPlay()
{
	Super.PostBeginPlay();

	// Find mini-game director
	foreach AllActors( class'Director', Director )
		break;

	// If there is a specified intro path, make it's end point match
	// the beginning of main path
	if ( Path_Intro != '' )
	{
		SplicePaths( Path_Intro, Path );
		PathToFly = Path_Intro;
	}
	else
		PathToFly = Path;

	SetPhysics(PHYS_Flying);

	// Load the player hit sounds
	HitSounds[0] = Sound'HPSounds.Quidditch_sfx.Q_Collision1';
	HitSounds[1] = Sound'HPSounds.Quidditch_sfx.Q_Collision2';
	HitSounds[2] = Sound'HPSounds.Quidditch_sfx.Q_Collision3';

	bHit = false;
}


//-------------------------------------------------------------------------------------------
// Cutscene support functions
//-------------------------------------------------------------------------------------------

function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
{
	// A cut-scene is commanding the quidditch player to do something; parse out command and
	// respond accordingly.

	local string	sActualCommand;
	local bool		bResult;


	sActualCommand = ParseDelimitedString( Command, " ", 1, false );

	// Respond to command
	if ( sActualCommand ~= "Capture" )
	{
		// Stop any flying so that cutscene can control pawn
		StopFlyingOnPath();
		bCapturedByCutScene = true;

		// Let HChar setup for cutscene
		return Super.CutCommand( Command, Cue, bFastFlag );
	}
	else if ( sActualCommand ~= "Release" )
	{
		// Let HChar cleanup after cutscene
		bResult = Super.CutCommand( Command, Cue, bFastFlag );

		// Restart normal behavior
		bCapturedByCutScene = false;
		return bResult;
	}
	else
	{
		return Super.CutCommand( Command, Cue, bFastFlag );
	}
}

//-------------------------------------------------------------------------------------------
// Appearance and Path support functions
//-------------------------------------------------------------------------------------------

function SetHouse( HouseAffiliation eOpponent )
{
	// Establishes which house the player is affiliated with.  If the
	// HouseToDisplayAs property is HA_DependsOnMatch, then the house
	// depends on which team the player is on and which house the opponent
	// team is; the non-opponent team is always Gryffindor house.  Otherwise,
	// The house is forced to be the value of the HouseToDisplayAs property.

	local int	iSkin;

	// Determine the house of the player
	if ( HouseToDisplayAs == HA_DependsOnMatch )
	{
		// Map the team to a house
		switch ( Team )
		{
			case TA_Gryffindor:	eHouse = HA_Gryffindor;	break;
			case TA_Opponent:	eHouse = eOpponent;		break;
			case TA_Neutral:	eHouse = HA_Gryffindor;	break;	// Equal to HA_Neutral
		};
	}
	else
	{
		eHouse = HouseToDisplayAs;
	}

	// Change the player's mesh and uniform to match the house
	if ( HouseDisplayInfo[ eHouse ].Mesh != None )
		Mesh = HouseDisplayInfo[ eHouse ].Mesh;
	for ( iSkin = 0; iSkin < QP_NUM_MULTISKINS; ++iSkin )
	{
		if ( HouseDisplayInfo[ eHouse ].MultiSkins[ iSkin ] != None )
			MultiSkins[ iSkin ] = HouseDisplayInfo[ eHouse ].MultiSkins[ iSkin ];
	}

	// Determine the player's sex
	eSex = HouseDisplayInfo[ eHouse ].Sex;
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

function FlyOnPath( name Path, optional int StartPoint )
{
	local InterpolationPoint	i;

	if ( Path != '' )
	{
		// Put character at selected point on its path
		i = FindPointOnPath( Path, StartPoint );

		if ( i != None )
		{
			SetLocation( i.Location );
//			SetRotation( i.Rotation );
			SetCollision( [NewColActors] true, [NewBlockActors] false, [NewBlockPlayers] true );
			bCollideWorld = false;
			bInterpolating = true;
			SetPhysics( PHYS_None );

			IM = Spawn( class'InterpolationManager', self );
			IM.Init( i.Next, 1.0, false );
		}

		if ( IM == None )
		{
			Log( Name$" couldn't find path "$Path );
		}
	}
	else
		Log( Name$" *** No path to fly on ***" );
}

function StopFlyingOnPath()
{
	local InterpolationManager	IM_ToStop;

	if ( IM != None )
	{
		IM_ToStop = IM;
		IM = None;		// Tells QuidditchPlayer.FinishInterpolation event that path ended early
		IM_ToStop.FinishedInterpolation( None );
	}
	bCollideWorld = true;
}


function SplicePaths( name Path1, name Path2, optional int iPath2SplicePointPos )
{
	// Moves the end point of path1 so that it matches up seamlessly with the
	// indicated point of path 2 (or lowest-numbered point, if not specifed)
	local InterpolationPoint	IP;
	local InterpolationPoint	EndPoint;
	local InterpolationPoint	SplicePoint;

	// Find end point of path1
	foreach AllActors( class'InterpolationPoint', IP, Path1 )
	{
		if ( IP.bEndOfPath )
		{
			EndPoint = IP;
			break;
		}
	}

	if ( EndPoint == None )
	{
		Log( "Couldn't find end point of path "$Path1 );
		return;
	}

	// Find splice point of path2
	SplicePoint = FindPointOnPath( Path2, iPath2SplicePointPos );
	if ( SplicePoint == None )
	{
		Log( "Couldn't find splice point of path "$Path2 );
		return;
	}

	// Make end point match up with splice point
	EndPoint.SetLocation( SplicePoint.Location );
	EndPoint.SetRotation( SplicePoint.Rotation );
	EndPoint.DesiredSpeed = SplicePoint.DesiredSpeed;
	EndPoint.StartControlPoint = SplicePoint.StartControlPoint;
	EndPoint.EndControlPoint = SplicePoint.EndControlPoint;
}

event FinishedInterpolation( InterpolationPoint Other )
{
	// This event happens when the quid player reaches the end-point on
	// its intro path.  Start regular path.
	PathToFly = Path;
	if ( IM != None )
	{
		IM = None;
		FlyOnPath( PathToFly );
	}
}

//-------------------------------------------------------------------------------------------
// Operational methods
//-------------------------------------------------------------------------------------------

function SetLookForTarget( Actor NewLookForTarget )
{
	// Notes what actor player should appear to "look for" when that actor isn't
	// visible.  If None, player will appear to look for something periodically.

	if ( WatchTarget == LookForTarget )
	{
		StopHeadLook();
		WatchTarget = None;
	}

	LookForTarget = NewLookForTarget;
}

function SetTargetTrackDist( float fNewTargetTrackDist )
{
	fTargetTrackDist = fNewTargetTrackDist;
}

function SetKickTargetClass( name NewKickTargetClassName )
{
	// Notes what kind of actor player should aim his kick at.  If None, player won't kick at anything.

	if ( WatchTarget == KickTarget )
	{
		StopHeadLook();
		WatchTarget = None;
	}

	KickTargetClassName = NewKickTargetClassName;
}

function UpdateKickTarget()
{
	// Finds nearest kick target and notes it and its distance.
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
		KickTarget.TakeDamage( 5, Self, Location, 100*Normal(TargetDir), 'Kicked' );
	}
}

function UpdateWatchTarget()
{
	// Decide what to watch with head
	local bool		bLookAtKickTarget;
	local vector	WatchOffset;

	if ( KickTarget != None )
		bLookAtKickTarget = frand() < 0.75;
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

function float GetHealth()
{
	return fHealth / 100.0;
}

event Tick( float DeltaTime )
{
	// Perform health recovery

	fHealth += HealthRecoveryRate * DeltaTime;
	if ( fHealth > 100.0 )
		fHealth = 100.0;

	if ( bStunned && fHealth > 66.666 )
		bStunned = false;
}

function Bump( Actor Other )
{
	// Quid player hit something, it stumbles, and imparts damage to other
	// actor, but only if it's a pawn.
	local Pawn	Target;

	Target = Pawn( Other );
	if ( !bHit && Other != LookForTarget && Target != None && !Target.bHidden )
	{
		if ( frand() < 0.8 )
			PlayAnim( 'React' );
		else
			PlayAnim( 'Bump' );

		PlaySound( HitSounds[ Rand( NUM_HIT_SOUNDS ) ], SLOT_Interact, 0.7, , 1000.0 );	// Radius makes sure player can be heard near by

		Target.TakeDamage( Damage, Self, Location, 100*Normal(Velocity), 'Collided' );

		Velocity = vect(0,0,1);
		bHit = true;
	}
}

function TakeDamage( int Damage, Pawn InstigatedBy, Vector HitLocation, 
					 Vector Momentum, name DamageType )
{
	// If got kicked, become stunned and drop pursuit
	if ( !bStunned )
	{
		StopHeadLook();
		WatchTarget = None;
		PlayAnim( 'Bump', , 0.2 );

		if ( IsInState( 'Pursue' ) )
		{
			if ( DamageType == 'Kicked' )
				fHealth -= 33.333;
			else if ( DamageType == 'Collided' )
				fHealth -= 9.0;
		}

		if ( fHealth <= 1.0 )
		{
			fHealth = 0.1;
			bStunned = true;
			fTimeToFallAway = Level.TimeSeconds + 3.0;
		}

		if ( DamageType != 'Collided' )	// Skip case where sound is played by pawn who bumped us
			PlaySound( HitSounds[ Rand( NUM_HIT_SOUNDS ) ], SLOT_Interact, 0.7, , 1000.0 );	// Radius makes sure player can be heard near by

		Super.TakeDamage( Damage, InstigatedBy, HitLocation, Momentum, DamageType );
	}
}

function HitWall( vector HitNormal, Actor Wall )
{
	// Quid player hit wall; stumble.
	if ( !bHit )
	{
		if ( frand() < 0.8 )
			PlayAnim( 'React' );
		else
			PlayAnim( 'Bump' );
		bHit = true;
	}
}

event AnimEnd()
{
	bHit = false;
}


//-------------------------------------------------------------------------------------------
// States
//
// WaitForIntro		- Waiting until triggered to fly on main path
// Fly				- No particular role; just flying on a path
// Pursue			- Chasing target while free-flying
// GetBackOnPath	- Flying on transitional path to get make on main path
//-------------------------------------------------------------------------------------------

auto state() WaitForIntro
{
	function BeginState()
	{
		LoopAnim( 'Hover' );
		PlayerHarry.ClientMessage( Name$' Waiting for Intro' );
		Log( Name$' Waiting for Intro' );
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$' Done waiting for Intro' );
		Log( Name$' Done waiting for Intro' );
	}

	function Trigger( Actor Other, Pawn EventInstigator )
	{
		// When triggered, this player's reaction is to stop waiting and either
		// begin flying on intro path or transition onto it's main flight path

		PlayerHarry.ClientMessage( "QuidditchPlayer "$ Name $ " Triggered" );

		if ( Path_Intro != '' )
		{
			PathToFly = Path_Intro;
			GotoState( 'Fly' );
		}
		else
		{
			PathToFly = Path;
			GotoState( 'GetBackOnPath' );
		}
	}

begin:
loop:
	FinishAnim();
	if ( frand() < 0.12 )
		LoopAnim( 'Look', , 0.5 );
	else
		LoopAnim( 'Hover', , 0.5 );

	goto 'loop';
}


state() Fly
{
	function BeginState()
	{
		LoopAnim( 'Fly_Forward' );
		FlyOnPath( PathToFly );
		PlayerHarry.ClientMessage( Name$' Begin Flying' );
		Log( Name$' Begin Flying' );
	}

	function EndState()
	{
		StopFlyingOnPath();
		LoopAnim( 'Hover', , 0.5 );
		PlayerHarry.ClientMessage( Name$' End Flying' );
		Log( Name$' End Flying' );
	}

begin:
loop:
	FinishAnim();
	if ( bCapturedByCutScene )
	{
		if ( VSize(Velocity) < 50.0 )
			LoopAnim( 'Hover', , 0.5 );
		else
			LoopAnim( 'Fly_Forward', , 0.5 );
	}
	else	// Normal interactive animations...
	{
		if ( bCaughtTarget )
			LoopAnim( 'Hold', , 0.1 );
		else if ( bStunned )
			LoopAnim( 'Stunned', , 0.5 );
		else if ( frand() < 0.4 )
			LoopAnim( 'Fly_Forward', , 0.5 );
		else if ( frand() < 0.8 )
			LoopAnim( 'Look', , 0.5 );
		else
			LoopAnim( 'Hover', , 0.5 );
	}

	goto 'loop';
}


state Pursue
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$' Begin Pursue' );
		Log( Name$' Begin Pursue' );
		LoopAnim( 'Fly_Forward', , 0.5 );
		SetPhysics( PHYS_Flying );

		fTargetTrackHorzOffset = RandRange( -TrackingOffsetRange_Horz, TrackingOffsetRange_Horz );
		fTargetTrackVertOffset = RandRange( -TrackingOffsetRange_Vert, TrackingOffsetRange_Vert );

		fTimeForNextOffset = Level.TimeSeconds;
		fTimeForNextKick = Level.TimeSeconds;
		fTimeForNextKickUpdate = Level.TimeSeconds;
		fTimeForNextWatchUpdate = Level.TimeSeconds;
	}

	event Tick( float DeltaTime )
	{
		local vector	X,Y,Z;
		local vector	TargetX, TargetY, TargetZ;	// Target's orientation vectors
		local vector	TargetTrackPoint;			// Point behind target player should close in on
		local vector	TargetDir;
		local float		TargetDist;
		local float		TargetSpeed;

		const	SlowdownRadius = 50;	// How close to target should pursuer begin slowing to match speed
		const	MaxSpeed = 1200;		// How fast can pursuer go

		if ( !bStunned )
			Global.Tick( DeltaTime );
		else
			Super.Tick( DeltaTime );	// Skip health recovery

		if ( bStunned && Level.TimeSeconds >= fTimeToFallAway )
			GotoState( 'GetBackOnPath' );

		if ( LookForTarget == None || LookForTarget.bHidden )
			GotoState( 'GetBackOnPath' );

		// If time to change target offset, do it
		if ( Level.TimeSeconds > fTimeForNextOffset )
		{
			fTargetTrackHorzOffset = RandRange( -TrackingOffsetRange_Horz, TrackingOffsetRange_Horz );
			fTargetTrackVertOffset = RandRange( -TrackingOffsetRange_Vert, TrackingOffsetRange_Vert );
			fTimeForNextOffset = Level.TimeSeconds + RandRange( 2.0, 4.0 );
		}

		// Determine track points
		GetAxes( LookForTarget.Rotation, TargetX, TargetY, TargetZ );	// Get Target's orientation vectors
		TargetTrackPoint = LookForTarget.Location
						 + TargetY * fTargetTrackHorzOffset
						 + TargetZ * (fTargetTrackVertOffset - CollisionHeight);	// Adjust for grab height

		// Update Rotation to point player at target's track point
		TargetDir = TargetTrackPoint - Location;
		DesiredRotation = Rotator( TargetDir );

		// Update acceleration and speed
		GetAxes( Rotation, X, Y, Z );
		if ( bStunned )
			AccelRate = 1100.0;
		else
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

//		Log( "Seeker: Track Dist = "$fTargetTrackDist$" Dist Error = "$TargetDist$" Air Speed = "$AirSpeed$" Vel = "$VSize( Velocity ) );

		// Do other AI if not stunned
		if ( !bStunned )
		{
			// If time to update the kick target do it
			if ( Level.TimeSeconds > fTimeForNextKickUpdate || Level.TimeSeconds > fTimeForNextKick )
			{
				UpdateKickTarget();
				fTimeForNextKickUpdate = Level.TimeSeconds + 1.0;
			}

			// If time to try a kick, do it
			if ( Level.TimeSeconds > fTimeForNextKick )
			{
				if ( KickTarget != None && KickTargetDist <= CollisionRadius * 3.0 )
					DoKick( KickTarget );
				fTimeForNextKick = Level.TimeSeconds + RandRange( 1.5, 3.0 );
			}

			// If time to update the watch target do it
			if ( Level.TimeSeconds > fTimeForNextWatchUpdate )
			{
				UpdateWatchTarget();
				fTimeForNextWatchUpdate = Level.TimeSeconds + RandRange( 1.0, 1.2 );
			}
		}
	}

	event AnimEnd()
	{
		bHit = false;

		// Hack to keep from prempting the Bump animation that preceeds the Stun animation
		if ( bStunned && (Level.TimeSeconds > fTimeToFallAway - (3.0+0.1)) )
			LoopAnim( 'Stunned', , 0.4 );
	}


begin:
	// Wait for when it's time to catch target
	Sleep( 120.0 );
	if ( bStunned )
		goto 'begin';

	// Let director know seeker is catching the snitch
	Director.Trigger( Self, None );

	// Start catch animation
	PlayAnim( 'Catch', , 0.1 );
	Sleep( 0.4 );			// Dead reckon when Catch animation has player's hand closing in around target

	// Attach target to player's hand
	LookForTarget.SetPhysics( PHYS_Trailer );
	LookForTarget.SetOwner( Self );
	LookForTarget.AttachToOwner( 'RightHand' );
	LookForTarget.bTrailerPrePivot = true;
	LookForTarget.PrePivot = vect( 3, -3, 0 );		// Offset snitch so it doesn't engulf hand

	// Wait for catch animation to end, then hold with target in hand
	FinishAnim();
	bCaughtTarget = true;
	LoopAnim( 'Hold', , 0.1 );

	// Transition player back to path now
	GotoState( 'GetBackOnPath' );
}


state GetBackOnPath extends Fly
{
	function BeginState()
	{
		local float					fDistance;
		local InterpolationPoint	i;
		local vector				X,Y,Z;

		local float					fClosestDistance;
		local InterpolationPoint	ClosestPoint;
		local int					iClosestPoint;

		local float					fSecondClosestDistance;
		local InterpolationPoint	SecondClosestPoint;
		local int					iSecondClosestPoint;


		PlayerHarry.ClientMessage( Name$' Begin GetBackOnPath' );
		Log( Name$' Begin GetBackOnPath' );

		if ( PathToFly == '' )
		{
			Log( Name$" No path to get back to" );
			GotoState( 'Fly' );
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

			SecondClosestPoint = None;
			fSecondClosestDistance = 999999.0;
			foreach AllActors( class'InterpolationPoint', i, PathToFly )
			{
				fDistance = VSize( Location - i.Location );
				if ( fDistance < fClosestDistance )
				{
					fSecondClosestDistance = fClosestDistance;
					SecondClosestPoint  = ClosestPoint;
					iSecondClosestPoint = iClosestPoint;

					ClosestPoint = i;
					fClosestDistance = fDistance;
					iClosestPoint = i.Position;
				}
				else if ( fDistance < fSecondClosestDistance )
				{
					SecondClosestPoint = i;
					fSecondClosestDistance = fDistance;
					iSecondClosestPoint = i.Position;
				}
			}

			if ( SecondClosestPoint == None )
			{
				fSecondClosestDistance = fClosestDistance;
				SecondClosestPoint  = ClosestPoint;
				iSecondClosestPoint = iClosestPoint;
			}
			iReturnPoint = iSecondClosestPoint;

			if ( SecondClosestPoint == None )
			{
				Log( Name$" Could not find an interpolation point on path '"$PathToFly$"' to get back to" );
				GotoState( 'Fly' );
			}
			else
			{
				// Create a path back to main path...
				// Set-up interpolation point at end of return path
				if ( ReturnPath[1] == None )
				{
					ReturnPath[1] = Spawn( class'DynamicInterpolationPoint' );
					ReturnPath[1].Tag = ReturnPath[1].Name;
					ReturnPath[1].Position = 1;
					ReturnPath[1].bEndOfPath = true;
				}
				ReturnPath[1].SetLocation( SecondClosestPoint.Location );
				ReturnPath[1].SetRotation( SecondClosestPoint.Rotation );
				ReturnPath[1].DesiredSpeed = SecondClosestPoint.DesiredSpeed;
				ReturnPath[1].StartControlPoint = SecondClosestPoint.StartControlPoint;
				ReturnPath[1].EndControlPoint = SecondClosestPoint.EndControlPoint;

				// Set-up interpolation point at beginning of return path
				if ( ReturnPath[0] == None )
				{
					ReturnPath[0] = Spawn( class'DynamicInterpolationPoint', , ReturnPath[1].Tag );
					ReturnPath[0].Position = 0;
					ReturnPath[0].bEndOfPath = false;
					ReturnPath[0].Next = ReturnPath[1];
					ReturnPath[0].Prev = ReturnPath[1];
					ReturnPath[1].Next = ReturnPath[0];
					ReturnPath[1].Prev = ReturnPath[0];
				}

				ReturnPath[0].SetLocation( Location );
				ReturnPath[0].SetRotation( Rotation );
				ReturnPath[0].DesiredSpeed = VSize( Velocity );
				if ( ReturnPath[0].DesiredSpeed < 50 )
					ReturnPath[0].DesiredSpeed = 50;

				GetAxes( Rotation, X, Y, Z );
				ReturnPath[0].StartControlPoint =  1000.0 * X;
				ReturnPath[0].EndControlPoint   = -1000.0 * X;

				// Start flying on return path;
				SetCollision( [NewColActors] true, [NewBlockActors] false, [NewBlockPlayers] true );
				bCollideWorld = false;
				bInterpolating = true;
				SetPhysics( PHYS_None );

				IM = Spawn( class'InterpolationManager', self );
				IM.Init( ReturnPath[1], 1.0, false );
			}
		}
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$' End GetBackOnPath' );
		Log( Name$' End GetBackOnPath' );
		StopFlyingOnPath();
	}

	event FinishedInterpolation( InterpolationPoint Other )
	{
		// This event happens when the quid player reaches the end-point on
		// the return path.  Start regular seeking again.
		Log( Name$" Got onto path '"$PathToFly$"'" );
		if ( IM != None )
		{
			IM = None;
			GotoState( 'Fly' );
		}
	}
}

defaultproperties
{
	Team=TA_Gryffindor
	HouseToDisplayAs=HA_DependsOnMatch
	eSex=SX_Neutral
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh'
	ShadowClass=class'BroomShadow'
	bAlignBottom=false
	RotationRate=(Yaw=50000)
	RotationRate=(Roll=2000)
	RotationRate=(Pitch=24000)

	fTargetTrackDist=200.0
	TrackingOffsetRange_Horz=100
	TrackingOffsetRange_Vert=75

	bRotateToDesired=true

	HealthRecoveryRate=8.0
	Damage=2
}
