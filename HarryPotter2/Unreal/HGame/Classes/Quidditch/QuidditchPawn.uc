//================================================================================
// QuidditchPawn  -- An non-char object in Quidditch that can fly around a path
//================================================================================
class QuidditchPawn extends HPawn;

var Harry					PlayerHarry;			// Actor to channel ClientMessages through
var QuidditchCommentator	Commentator;			// Source of spoken game commentary, if any

var InterpolationManager	IM;
var(Quidditch) name			Path[8];				// Interpolation paths to follow
const						MaxPaths = 8;			// Max number of interpolation paths there can be
var int						CurPath;				// Which path is pawn flying on now

var DynamicInterpolationPoint	ReturnPath[2];		// Path to follow to get back on main path
var int							iReturnPoint;		// Position Id of point on main path that player is returning to
var bool						bGettingBackOnPath;	// Pawn is flying on return path right now

var(Quidditch) bool			bSwitchPathsOnTrigger;	// Whether pawn should switch paths when triggered
var(Quidditch) bool			bUseTransitionPaths;	// Whether pawn should follow transition paths between path switches
var(Quidditch) bool			bRandomizePaths;		// Whether pawn switches to a random path (rather than sequential)
var(Quidditch) float		fHideTime;				// How long to hide during path switch (0 = none)

var(Quidditch) float		fSpeedChangeFactor;		// Scale factor to multiply speed by on each speed change; 1.0 means never change
var(Quidditch) float		fSpeedChangePeriod;		// How often to change speed (after changes start); seconds
var(Quidditch) float		fSpeedChangeFirstTime;	// When to start periodic speed changes; seconds
var(Quidditch) int			MaxSpeedChanges;		// Maximum number of times speed can be changed
var float					fSpeedChangeLastTime;	// When to stop periodic speed changes; seconds
var float					fSpeedChangeNextTime;	// When next speed change should occur

var Pawn					Target;					// Which pawn should this pawn seek
var(Quidditch) float		fLaunchProximity;		// How close to target should pawn be to launch at target
var(Quidditch) float		fPursuitAbortDistance;	// How far from target should pawn be to break off pursuit
var(Quidditch) float		fPursuitTimeLimit;		// After how many seconds should pawn be to break off pursuit
var float					Dist;					// How far from target currently
var(Quidditch) float		fLaunchSpeed;			// How fast should pawn launch at target
var(Quidditch) float		fLaunchVelocityCompensation;	// What percent of target's velocity should pawn match?
var bool					bProjectile;			// Now acting as a projectile
var float					fLifeSpanAsProjectile;	// Maximum time pawn can stay as a projectile during targeting
const 						fDefaultLifeSpanAsProjectile = 10.0;	// Used if not specified during launch
var Actor					Emitter;				// Which actor turned this pawn into a projectile
var bool					bRecycle;				// Restart on path after used as a projectile

var bool					bMine;					// Now acting as a mine
var float					fLifeSpanAsMine;		// Maximum time pawn can stay as a mine during laying in wait
const 						fDefaultLifeSpanAsMine = 10.0;	// Used if not specified during deployment
var vector					DeployedLocation;		// Where was mine when first deployed; used for bobbing

var(Quidditch) class<ParticleFX>	ParticleTrail;	// Class of particles to trail pawn
var ParticleFX				Trail;					// The particle trail object itself

var(Quidditch) class<Halo>	HaloClass;				// Class of halo to surround pawn
var Halo					Halo;					// Halo sprite attached to pawn

var(Quidditch) class<ParticleFX>	ExplosionFX;	// Class of particles used for mine explosion

var(Quidditch) Sound		FlyingSound;			// Sound pawn makes while flying around, visibly
var(Quidditch) Sound		PursuitSound;			// Sound pawn makes while pursuing target
var(Quidditch) Sound		ExplosionSound;			// Sound pawn makes when it explodes


//-------------------------------------------------------------------------------------------
// PreBeginPlay(), PostBeginPlay(), Destroyed()
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

	// Find actor that channels ClientMessages
	foreach AllActors( class'Harry', PlayerHarry )
		break;
}


function PostBeginPlay()
{
	local InterpolationPoint	i;

	Super.PostBeginPlay();

	// Find commentator
	foreach AllActors( class'QuidditchCommentator', Commentator )
		break;

	SetPhysics(PHYS_Flying);

	bProjectile = false;
	bMine = false;
	bRecycle = false;
	Emitter = None;
	Target = None;
	bGettingBackOnPath = false;

	fSpeedChangeNextTime = fSpeedChangeFirstTime;
	fSpeedChangeLastTime = fSpeedChangePeriod * (MaxSpeedChanges-1) + fSpeedChangeFirstTime;

	if ( ParticleTrail != None )
	{
		Trail = Spawn( ParticleTrail, self, , location, rot(0,0,0) );
		Trail.SetPhysics( PHYS_Trailer );
	}

	if ( HaloClass != None )
		Halo = Spawn( HaloClass, self, , location, rot(0,0,0) );

	CurPath = -1;

	// Start on a path
	ChooseNextPath();
	if ( fHideTime > 0.0 )
	{
		Hide();
		SetTimer( fHideTime, false );
	}
	else
	{
		FlyOnPath( Path[CurPath] );
		Show();
	}
}


function Destroyed()
{
	// Destructor: destroy objects associated with pawn

	// Make sure interpolation manager is destroyed
	StopFlyingOnPath();

	// Make sure ParticleTrail shuts down (will self-destruct)
	if ( Trail != None )
		Trail.Shutdown();

	// Destroy any attached halo
	if ( Halo != None )
		Halo.Destroy();

	Super.Destroyed();
}


//-------------------------------------------------------------------------------------------
// Cutscene support functions
//-------------------------------------------------------------------------------------------

function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
{
	// A cut-scene is commanding the quidditch pawn to do something; parse out command and
	// respond accordingly.

	local string	sActualCommand;
	local bool		bResult;


	sActualCommand = ParseDelimitedString( Command, " ", 1, false );

	// Respond to command
	if ( sActualCommand ~= "Capture" )
	{
		// Stop any flying so that cutscene can control pawn
		StopFlyingOnPath();

		// Let HPawn setup for cutscene
		return Super.CutCommand( Command, Cue, bFastFlag );
	}
	else if ( sActualCommand ~= "Release" )
	{
		// Let HPawn cleanup after cutscene
		bResult = Super.CutCommand( Command, Cue, bFastFlag );

		// Restart normal behavior
		SwitchPaths();
		return bResult;
	}
	else
	{
		return Super.CutCommand( Command, Cue, bFastFlag );
	}
}

//-------------------------------------------------------------------------------------------
// Path and visibility support functions
//-------------------------------------------------------------------------------------------

function Hide()
{
	bHidden = true;

	if ( FlyingSound != None )
		StopSound( FlyingSound, SLOT_Misc );
	if ( PursuitSound != None )
		StopSound( PursuitSound, SLOT_Misc );

	if ( Trail != None )
		Trail.EnableEmission( false );

	if ( Halo != None )
		Halo.bHidden = true;
}

function Show()
{
	bHidden = false;

	if ( FlyingSound != None )
		PlaySound( FlyingSound, SLOT_Misc, 1.0, , 10000.0 );	// Radius makes sure pawn can be heard far away

	if ( Trail != None )
		Trail.EnableEmission( true );

	if ( Halo != None )
		Halo.bHidden = false;
}

function bool TargetCanSeeMe( Pawn Target )
{
	// Returns true if target can see this pawn.  Different from the usually
	// CanSee function in that if the target is a player, the question changes
	// to whether the players' camera can see this pawn.

	local Harry		PlayerTarget;
	local bool		bCanSee;
	
	PlayerTarget = Harry( Target );
	if ( PlayerTarget != None )
		bCanSee = PlayerTarget.Cam.CanSee( Self );
	else
		bCanSee = Target.CanSee( Self );

	return bCanSee;
}

function ChooseNextPath()
{
	// Randomly picks another path.  Will only choose those that are actually
	// defined; will bail out if no paths are defined
	local int	Skip;
	local int	BailOut;

	if ( bRandomizePaths )
		Skip = frand() * (MaxPaths-1) + 1;
	else
		Skip = 1;

	do
	{
		// Skip to next defined path
		BailOut = MaxPaths;
		do
		{
			CurPath++;
			if ( CurPath >= MaxPaths )
				CurPath = 0;
			--BailOut;
		}
		until ( Path[CurPath] != '' || BailOut <= 0 );
		--Skip;
	}
	until ( Skip <= 0 || BailOut <= 0 )

	Log( Name$" chose new path: "$Path[CurPath]$"." );
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
		// Put pawn at first point on its path
		i = FindPointOnPath( Path, StartPoint );

		if ( i != None )
		{
			SetLocation( i.Location );
//			SetRotation( i.Rotation );
			SetCollision( true, false, false );
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
		Log( Name$" Started interpolation with "$IM.Name$":"$IM );
	}
}

function StopFlyingOnPath()
{
	local InterpolationManager	IM_ToStop;

	bCollideWorld = true;
	if ( IM != None )
	{
		IM_ToStop = IM;
		IM = None;		// Tells QuidditchPawn.FinishInterpolation event that path ended early
		Log( Name$" Stopping interpolation with "$IM_ToStop.Name$":"$IM_ToStop );
		IM_ToStop.FinishedInterpolation( None );
		Log( Name$" Stopped interpolation with "$IM_ToStop.Name$":"$IM_ToStop );
	}
}

function ApplySpeedChange( float fSpeedChange )
{
	// Adjusts (multiplies) all speed parameters by the factor specified by
	// the fSpeedChange parameter.
	local int					iPath;
	local InterpolationPoint	IP;

	// Adjust all InterpolationPoints on all paths
	for ( iPath = 0; iPath < MaxPaths; ++iPath )
	{
		if ( Path[ iPath ] != '' )
		{
			foreach AllActors( class'InterpolationPoint', IP, Path[ iPath ] )
				IP.DesiredSpeed *= fSpeedChange;
		}
	}

	// Adjust IP speed override, if set
	if ( IPSpeed != 0.0 )
		IPSpeed *= fSpeedChange;

	// Adjust projectile and pursuit speeds
	fLaunchSpeed *= fSpeedChange;
}

function CheckIfTimeForSpeedChange()
{
	// Checks to see if a speed change should be done, and does it if necessary.

	if (    (fSpeedChangeFactor != 1.0)
		 && (MaxSpeedChanges > 0)
		 && (fSpeedChangeNextTime <= fSpeedChangeLastTime)
		 && (Level.TimeSeconds >= fSpeedChangeNextTime) )
	{
		// Apply change and set alarm for next change
		ApplySpeedChange( fSpeedChangeFactor );
		fSpeedChangeNextTime += fSpeedChangePeriod;

		PlayerHarry.ClientMessage( Name$": Speeds changed." );
		Log( Name$": Speeds changed." );
	}
}

event Tick( float DeltaTime )
{
	Super.Tick( DeltaTime );

	// Update snitch base speeds
	CheckIfTimeForSpeedChange();
}


//-------------------------------------------------------------------------------------------
// Path sequencing
//-------------------------------------------------------------------------------------------

function Trigger( Actor Other, Pawn EventInstigator )
{
	// This object can receive a trigger when it reaches some marked point on
	// it's current interpolation path or via any other trigger mechanism.
	// The default reaction is to stop the current path, which in turn will
	// change paths according to the defined properties.

	if ( bSwitchPathsOnTrigger )
		IM.FinishedInterpolation( None );
}

event FinishedInterpolation(InterpolationPoint Other)
{
	// This event happens when the object reaches an end-point on the current path.
	// The default reaction is to change paths according to the defined properties.

	// If not being forced to stop, switch paths
	if ( !bCollideWorld )	// Would be true if commanded to stop flying on path
		SwitchPaths();
}

function SwitchPaths()
{
	ChooseNextPath();

	if ( fHideTime > 0.0 )
	{
		Hide();
		SetTimer( fHideTime, false );
	}
	else if ( bUseTransitionPaths )
	{
		GotoState( 'GetBackOnPath' );	// Fly on transition path to chosen path
	}
	else
	{
		FlyOnPath( Path[CurPath] );
	}
}

function Timer()
{
	// Timer expires when hide-time has elapsed.  Pawn can hide during path switches.
	// Time to start pawn on current path and reveal it.

	FlyOnPath( Path[CurPath] );
	Show();
}


//-------------------------------------------------------------------------------------------
// Projectile support
//-------------------------------------------------------------------------------------------

function SeekTarget( Pawn NewTarget, optional float fNewLaunchProximity )
{
	Target = NewTarget;
	if ( fNewLaunchProximity > 0 )
		fLaunchProximity = fNewLaunchProximity;

	if ( IsInState( 'Pursue' ) )
		GotoState( 'GetBackOnPath' );
	else if ( !(IsInState( 'Seeking' ) || IsInState( 'GetBackOnPath' )) )
		GotoState( 'Seeking' );
}

function SetLaunchParameters( float fSpeed, float fVelocityCompensation )
{
	fLaunchSpeed = fSpeed;
	fLaunchVelocityCompensation = fVelocityCompensation;
}

function LaunchAtTarget( Pawn NewTarget, optional Actor NewEmitter, optional float fLife )
{
	// Send pawn off as a projectile towards target

	local Vector	TargetingVelocity;		// Direction and speed to be a projectile

	TargetingVelocity = NewTarget.Location - Location;
	TargetingVelocity = Normal( TargetingVelocity );
	TargetingVelocity *= fLaunchSpeed;
	TargetingVelocity += fLaunchVelocityCompensation * NewTarget.Velocity;	// Anticipate target's movement, to some percentage
	ConvertIntoProjectile( NewEmitter, TargetingVelocity, fLife );

	GotoState( 'Targeting' );
}

function ConvertIntoProjectile( Actor NewEmitter, Vector NewVelocity, optional float fLife )
{
	StopFlyingOnPath();

	SetPhysics( PHYS_Projectile );
	Emitter = NewEmitter;
	Velocity = NewVelocity;
	Acceleration = vect( 0, 0, 0 );
	bProjectile = true;
	SetCollision( [NewBlockActors] false, [NewBlockPlayers] false );	// Ensures the Touch event will happen

	// Limit life in case it never hits anything
	if ( fLife == 0 )
		fLifeSpanAsProjectile = fDefaultLifeSpanAsProjectile;
	else
		fLifeSpanAsProjectile = fLife;
}

function FinishProjectile()
{
	// Called when this pawn, acting as a projectile, finishes it's trajectory

	PlayerHarry.ClientMessage( Name$" done targeting" );
	Log( Name$" done targeting" );

	if ( bRecycle )
	{
		// Make it switch paths (and hide for a while, if set to)
		SetPhysics(PHYS_Flying);

		bProjectile = false;
		Emitter = None;
		SwitchPaths();
		if ( IsInState( 'Targeting' ) )
			GotoState( 'Seeking' );
	}
	else
	{
		// Not needed anymore, destroy it
		Destroy();
	}
}

function Touch( Actor Other )
{
	if ( bProjectile && Other != Emitter )
		FinishProjectile();
	else if ( bMine && Other == Target )
		ExplodeMine();
}

function HitWall( vector HitNormal, actor Wall )
{
	if ( bProjectile )
		FinishProjectile();
}

function TakeDamage( int Damage, Pawn InstigatedBy, Vector HitLocation, 
					 Vector Momentum, name DamageType )
{
	// You're average Quidditch pawn doesn't take damage, but other objects
	// may try to impart damage onto it; ignore damage.
}


//-------------------------------------------------------------------------------------------
// Mine support
//-------------------------------------------------------------------------------------------

function DeployAsMine( Pawn NewTarget, optional Actor NewEmitter, optional float fLife )
{
	StopFlyingOnPath();

	SetPhysics( PHYS_None );
	Emitter = NewEmitter;
	Target = NewTarget;
	Velocity = vect( 0, 0, 0 );
	Acceleration = vect( 0, 0, 0 );
	DeployedLocation = Location;
	bMine = true;
	SetCollision( [NewBlockActors] false, [NewBlockPlayers] false );	// Ensures the Touch event will happen

	// Limit life in case nothing ever hits it
	if ( fLife == 0 )
		fLifeSpanAsMine = fDefaultLifeSpanAsMine;
	else
		fLifeSpanAsMine = fLife;

	GotoState( 'LyingInWait' );
}

function ExplodeMine()
{
	// Play explosion sound
	if ( ExplosionSound != None  )
		PlaySound( ExplosionSound, SLOT_Interact, 1.0, , 10000.0 );	// Radius makes sure pawn can be heard far away

	// Spawn explosion particle effect
	if ( ExplosionFX != None )
		Spawn( ExplosionFX );

	FinishMine();
}

function FinishMine()
{
	// Called when this pawn, acting as a mine, finishes it's mission

	PlayerHarry.ClientMessage( Name$" done lying in wait" );
	Log( Name$" done lying in wait" );

	if ( bRecycle )
	{
		// Make it switch paths (and hide for a while, if set to)
		SetPhysics(PHYS_Flying);

		bMine = false;
		Emitter = None;
		SwitchPaths();
		GotoState( 'Cruising' );
	}
	else
	{
		// Not needed anymore, destroy it
		Destroy();
	}
}


//-------------------------------------------------------------------------------------------
// States
//
// Cruising			- Flying on path
// Seeking			- Flying on path, hoping to get near target
// Targeting		- Acquired target and is now a projectile
// LayingInWait		- Deployed as a mine and waiting for target
// Pursue			- Chasing target while free-flying
// GetBackOnPath	- Flying on transitional path to get make on main path
//-------------------------------------------------------------------------------------------

state() Cruising
{
Begin:
	PlayerHarry.ClientMessage( Name$" now cruising" );
	Log( Name$" now cruising" );

Loop:
	Sleep( 0.1 );
	goto 'Loop';
}

state() Seeking
{
Begin:
	PlayerHarry.ClientMessage( Name$" now seeking "$Target.Name );
	Log( Name$" now seeking "$Target.Name );

Loop:
	if ( Target != None )
	{
		// If target is close enough, and target can see this pawn (no fair
		// shooting in the back!), then pursue target
		Dist = vsize( Location - Target.Location );
		if ( Dist <= fLaunchProximity && TargetCanSeeMe( Target ) )
		{
//			PlayerHarry.ClientMessage( Name$" now targeting "$Target.Name );
//			Log( Name$" now targeting "$Target.Name );
//			LaunchAtTarget( Target );

			Log( Name$" IPSpeed, Velocity: "$IPSpeed$", "$VSize(Velocity) );
			StopFlyingOnPath();
			GotoState( 'Pursue' );
		}
	}

//	Log( Name$" Seek IPSpeed, Velocity, Location: "$IPSpeed$", "$VSize(Velocity)$", "$Location );

	Sleep( 0.1 );
	goto 'Loop';
}

state() Targeting
{
	function EndState()
	{
		if ( fLifeSpanAsProjectile != 0 )
			SetTimer( 0.0, false );
	}

	function Timer()
	{
		// Pawn exhausted it's maximum time as a projectile without hitting anything
		if ( bProjectile )
			FinishProjectile();
	}

Begin:
	PlayerHarry.ClientMessage( Name$" now targeting "$Target.Name );
	Log( Name$" now targeting "$Target.Name );

	if ( fLifeSpanAsProjectile != 0 )
		SetTimer( fLifeSpanAsProjectile, false );

Loop:
	Sleep( 0.1 );
	goto 'Loop';
}

state() LyingInWait
{
	function EndState()
	{
		if ( fLifeSpanAsMine != 0 )
			SetTimer( 0.0, false );
	}

	function Timer()
	{
		// Pawn exhausted it's maximum time as a mine without being hit
		if ( bMine )
			FinishMine();
	}

	event Tick( float DeltaTime )
	{
		local vector	BobOffset;

		Super.Tick( DeltaTime );

		// Update bob
		BobOffset.X =  2.5 * sin(4.5 * Level.TimeSeconds);
		BobOffset.Y =  2.5 * sin(3.5 * Level.TimeSeconds);
		BobOffset.Z = 11.0 * sin(8.0 * Level.TimeSeconds);

		SetLocation( DeployedLocation + BobOffset );
	}

Begin:
	PlayerHarry.ClientMessage( Name$" now lying in wait for "$Target.Name );
	Log( Name$" now lying in wait for "$Target.Name );

	if ( fLifeSpanAsMine != 0 )
		SetTimer( fLifeSpanAsMine, false );

Loop:
	Sleep( 0.1 );
	goto 'Loop';
}

state Pursue
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$' Begin Pursue' );
		Log( Name$' Begin Pursue at speed '$fLaunchSpeed );
		SetPhysics( PHYS_Flying );

		// Intensify flying sound
		if ( PursuitSound == None || PursuitSound == FlyingSound )
		{
			if ( FlyingSound != None )
				PlaySound( FlyingSound, SLOT_Misc, 2.0, , 10000.0, 1.5 );	// Radius makes sure pawn can be heard far away
		}
		else
			PlaySound( PursuitSound, SLOT_Misc, 1.0, , 10000.0 );	// Radius makes sure pawn can be heard far away

		// Comment on bludger
		if ( Commentator != None )
			Commentator.SayComment( QC_BludgerPursuit, , true );

		// Setup time-out timer
		if ( fPursuitTimeLimit != 0 )
			SetTimer( fPursuitTimeLimit, false );
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$' End Pursue' );

		// Cancel time-out timer
		if ( fPursuitTimeLimit != 0 )
			SetTimer( 0.0, false );

		// Return flying sound to normal
		if ( FlyingSound != None )
			PlaySound( FlyingSound, SLOT_Misc, 1.0, , 10000.0 );	// Radius makes sure pawn can be heard far away
		else if ( PursuitSound != None )
			StopSound( PursuitSound, SLOT_Misc );
	}

	event Tick( float DeltaTime )
	{
		local vector	TargetDir;
		local vector	X,Y,Z;

		Super.Tick( DeltaTime );

		// Update snitch base speeds
		CheckIfTimeForSpeedChange();

		// If target is gone [or out of view], abort pursuit
		if ( Target == None || Target.bHidden /*|| !CanSee( Target )*/ )
			GotoState( 'GetBackOnPath' );

		// If target has outrun pawn, abort pursuit
		Dist = vsize( Location - Target.Location );
//		Log( Name$" Velocity, Location, Dist: "$VSize(Velocity)$", "$Dist$", "$Location );
		if ( Dist > fPursuitAbortDistance )
		{
			PlayerHarry.ClientMessage( Name$" aborted pursuit" );
			Log( Name$" aborted pursuit" );

			// Comment on bludger miss
			if ( Commentator != None )
				Commentator.SayComment( QC_BludgerMiss, , true );

			GotoState( 'GetBackOnPath' );
		}

		// Update Rotation to point at target
		TargetDir = Target.Location - Location;
		DesiredRotation = Rotator( TargetDir );

		// Update acceleration and speed
		GetAxes( Rotation, X, Y, Z );
		AccelRate = 5000.0;
		Acceleration = AccelRate * X;	// Make speed changes have instant effect
		Velocity = fLaunchSpeed * X;
		AirSpeed = fLaunchSpeed;
		DesiredSpeed = AirSpeed;
	}

	function Touch( Actor Other )
	{
		// Hit something, stop pursuit and return to seeking target again

		if ( Other == Target )
		{
			// Comment on bludger hit
			if ( Commentator != None )
				Commentator.SayComment( QC_BludgerHit, , true );
		}
		else
		{
			// Comment on bludger miss
			if ( Commentator != None )
				Commentator.SayComment( QC_BludgerMiss, , true );
		}

		GotoState( 'GetBackOnPath' );
	}

	function Timer()
	{
		// Pawn exhausted it's maximum time in pursuit without hitting anything

		// Comment on bludger miss
		if ( Commentator != None )
			Commentator.SayComment( QC_BludgerMiss, , true );

		GotoState( 'GetBackOnPath' );
	}
}


state GetBackOnPath
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

		if ( Path[CurPath] == '' )
		{
			if ( Target != None )
				GotoState( 'Seeking' );
			else
				GotoState( 'Cruising' );
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
			foreach AllActors( class'InterpolationPoint', i, Path[CurPath] )
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
				Log( Name$' Could not find an interpolation point to get back to' );
				FlyOnPath( Path[CurPath] );
				if ( Target != None )
					GotoState( 'Seeking' );
				else
					GotoState( 'Cruising' );
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
//				ReturnPath[0].DesiredSpeed = ReturnPath[1].DesiredSpeed;

				GetAxes( Rotation, X, Y, Z );
				ReturnPath[0].StartControlPoint =  1000.0 * X;
				ReturnPath[0].EndControlPoint   = -1000.0 * X;

				// Start flying on return path;
				SetCollision( true, false, false );
				bCollideWorld = false;
				bInterpolating = true;
				SetPhysics( PHYS_None );
				Log( Name$" return path speeds: "$ReturnPath[0].DesiredSpeed$", "$ReturnPath[1].DesiredSpeed );
				Log( Name$" IPSpeed, Velocity: "$IPSpeed$", "$VSize(Velocity) );
//				IPSpeed = fLaunchSpeed;
//				Velocity = fLaunchSpeed * X;
//				Acceleration = 200000.0 * X;

				bGettingBackOnPath = true;
				IM = Spawn( class'InterpolationManager', self );
				Log( Name$" Starting GBOP interpolation with "$IM.Name$":"$IM );
				IM.Init( ReturnPath[1], 1.0, false );
			}
		}
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$' End GetBackOnPath' );
		Log( Name$' End GetBackOnPath' );

		if ( bGettingBackOnPath )
		{
			bGettingBackOnPath = false;
			StopFlyingOnPath();
		}
	}

	event Tick( float DeltaTime )
	{
		Super.Tick( DeltaTime );

		// Update snitch base speeds
		CheckIfTimeForSpeedChange();

//		Log( Name$" GBOP Velocity, Location: , "$VSize(Velocity)$", , "$Location );
	}

	event FinishedInterpolation( InterpolationPoint Other )
	{
		// This event happens when the quidditch pawn reaches the end-point on
		// the return path.  Start regular seeking again.
		if ( IM != None )
		{
			Log( Name$" Finished interpolation with "$IM.Name$":"$IM );
			IM = None;
			bGettingBackOnPath = false;
			FlyOnPath( Path[CurPath], iReturnPoint );
			if ( Target != None )
				GotoState( 'Seeking' );
			else
				GotoState( 'Cruising' );
			Log( Name$" Finished interpolation event with "$IM.Name$":"$IM );
		}
	}
}


defaultproperties
{
	DrawScale=1.0
	bAlignBottom=false
	bSwitchPathsOnTrigger=false
	bUseTransitionPaths=false
	bRandomizePaths=false
	fHideTime=0.0
	bBlockActors=false
	bBlockPlayers=false
	InitialState=Cruising
	RotationRate=(Yaw=50000)
	RotationRate=(Roll=2000)
	RotationRate=(Pitch=24000)
	bRotateToDesired=true
	fSpeedChangeFactor=1.0
	fSpeedChangePeriod=60.0
	fSpeedChangeFirstTime=180.0
	MaxSpeedChanges=0
	fLaunchProximity=1200.0
	fPursuitAbortDistance=1300.0
	fPursuitTimeLimit=4.0
	fLaunchSpeed=700.0
	fLaunchVelocityCompensation=0.30
}
