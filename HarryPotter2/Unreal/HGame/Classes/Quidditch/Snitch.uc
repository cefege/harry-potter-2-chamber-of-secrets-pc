//=============================================================================
// Snitch  -- The little golden flying ball that Harry chases around in Quidditch
//=============================================================================
class Snitch extends QuidditchPawn;

var BroomHoopTrail		HoopTrail;

var(Quidditch) bool		bHasHoopTrail;			// Whether or not snitch has a trail of hoops behind it

var(Quidditch) float	fHoopSpacing;			// Seconds between hoop emissions
var(Quidditch) int		HoopTrailLen;			// How many hoops define touchable portion of trail
var(Quidditch) int		InitialHoopTrailEnd;	// How many hoops back from snitch is the initial end of trail
var(Quidditch) bool		bHoopsVisible;			// Whether or not to display hoops visibly in hoop trail
var(Quidditch) bool		bTrackProgress;			// Whether or not hoop trail tracks player progress

var float				fApparentScale;			// Current size to draw composite snitch (snitch + halo)
var float				fNormalScale;			// Normal snitch draw scale
var float				fNormalHaloScale;		// Normal draw scale of snitch's halo

var bool				bJerking;				// Whether jerk effect is on or not
var float				fTimeForNextJerk;		// When next to update jerk offset
var Vector				DestJerkOffset;			// Goal jerk offset to transition to
var Vector				JerkOffset;				// Current jerk offset (transition in progress)
var Vector				NormalHaloPrePivot;		// Where halo is when jerk offset is zero
var Vector				NormalTrailPrePivot;	// Where Partical Trail emits from when jerk offset is zero

var bool				bFlashing;				// Whether flashing halo effect is on or not
var float				fTimeForNextFlash;		// When next to toggle halo visibility


//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
	if ( Mesh == None )
		Mesh = SkeletalMesh'HPModels.skSnitchMesh';

	if ( ParticleTrail == None )
		ParticleTrail = class'Snitch_FX';

	// Overrided editor settings (hopefully temp)
	bHasHoopTrail = true;
	fHoopSpacing = 1.5;
	HoopTrailLen = 2;
	InitialHoopTrailEnd = 0;
	bHoopsVisible = true;
	bFlashing = false;

	Super.PostBeginPlay();

	if ( bHasHoopTrail )
	{
		HoopTrail = Spawn( class'BroomHoopTrail', , 'HoopTrail', , );
		HoopTrail.Emitter = Self;
		HoopTrail.SetTrailProperties( fHoopSpacing, HoopTrailLen, InitialHoopTrailEnd,
									  bHoopsVisible, bTrackProgress );
		HoopTrail.GotoState( 'TrailOn' );
	}
	else
		HoopTrail = None;

	LoopAnim( 'Flap' );

	// Prepare jerking effect
	bJerking=false;
	DestJerkOffset = PrePivot;
	JerkOffset = PrePivot;

	if ( Halo != None )
	{
		NormalHaloPrePivot = Halo.PrePivot;
		Halo.bTrailerPrePivot = true;
	}
	if ( Trail != None )
	{
		NormalTrailPrePivot = Trail.PrePivot;
		Trail.bTrailerPrePivot = true;
	}

	// Set-up apparent draw scale
	fApparentScale = 1.0;
	fNormalScale = DrawScale;
	if ( Halo != None )
		fNormalHaloScale = Halo.DrawScale;
	else
		fNormalHaloScale = 1.0;
}


function Hide()
{
	Super.Hide();

	if ( HoopTrail != None )
		HoopTrail.GotoState( 'TrailOff' );
}

function Show()
{
	Super.Show();

	if ( HoopTrail != None )
		HoopTrail.GotoState( 'TrailOn' );
}


function SetApparentScale( float fNewScale )
{
	// Change the apparent size of both the snitch and it's halo
	// to simulate difference in distance.
	fApparentScale = fNewScale;
	DrawScale = fNormalScale * fApparentScale;
	if ( Halo != None )
		Halo.DrawScale = fNormalHaloScale * fApparentScale;
}

function SetJerking( bool bOn )
{
	// Enables/disables jerking effect

	if ( bJerking != bOn )
	{
		if ( bOn )	// Turn on
			fTimeForNextJerk = Level.TimeSeconds;
		else		// Turn off
			DestJerkOffset = vect(0,0,0);

		bJerking = bOn;
	}
}

function SetFlashing( bool bOn )
{
	// Enables/disables flashing Halo effect

	if ( bFlashing != bOn )
	{
		if ( bOn )	// Turn on
			fTimeForNextFlash = Level.TimeSeconds;
		else		// Turn off
		{
			if ( Halo != None )
				Halo.bHidden = false;
		}

		bFlashing = bOn;
	}
}

event Tick( float DeltaTime )
{
	const			JerkSpeed = 400;
	const			FlashPeriod = 0.05;
	local Vector	Dir;
	local float		Dist;

	// If time to update jerk position, do it
	if ( bJerking && Level.TimeSeconds > fTimeForNextJerk )
	{
		DestJerkOffset.x = RandRange( -40, 40 );
		DestJerkOffset.y = RandRange( -40, 40 );
		DestJerkOffset.z = RandRange( -40, 40 );
		fTimeForNextJerk = Level.TimeSeconds + RandRange( 0.4, 1.2 );
	}

	// Update PrePivot to move towards jerk offset at a constant speed
	Dir = DestJerkOffset - JerkOffset;
	Dist = VSize( Dir );
	if ( Dist > 0.0 )
	{
		if ( Dist > 0.01 )
			JerkOffset += Normal( Dir ) * Min( Dist, JerkSpeed * DeltaTime );
		else
			JerkOffset = DestJerkOffset;

		// Offset all attached effects
		// (Note: hoop trail not offset)
		if ( Halo != None )
			Halo.PrePivot = NormalHaloPrePivot + JerkOffset;
		if ( Trail != None )
			Trail.PrePivot = NormalTrailPrePivot + JerkOffset;
	}

	PrePivot = JerkOffset << Rotation;	// Transform into local space, like trailers do;
										// Have to do it every tick because rotation changes

	// If time to flash, do it
	if ( bFlashing && Level.TimeSeconds > fTimeForNextFlash )
	{
		if ( Halo != None )
			Halo.bHidden = !Halo.bHidden;
		fTimeForNextFlash += FlashPeriod;
		if ( fTimeForNextFlash < Level.TimeSeconds )
			fTimeForNextFlash = Level.TimeSeconds;
	}
}


//-------------------------------------------------------------------------------------------
// States
//
// Cruising		- Flying on path, jerking around
//-------------------------------------------------------------------------------------------

state() Cruising
{
	function BeginState()
	{
		SetJerking( true );
	}

	function EndState()
	{
		SetJerking( false );
		SetApparentScale( 1.0 );
		SetFlashing( false );
	}
}


defaultproperties
{
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skSnitchMesh'
	ParticleTrail=class'Snitch_FX'
	HaloClass=class'Snitch_Halo'
	FlyingSound=Sound'HPSounds.Quidditch_sfx.Q_Snitch_Loop'
	Path(0)=IPSnitch
	IPSpeed=800;
	fSpeedChangeFactor=1.0	// Never slowdown
	fSpeedChangePeriod=60.0
	fSpeedChangeFirstTime=240.0
	MaxSpeedChanges=3
    CollisionHeight=50
    CollisionRadius=50
	bHasHoopTrail=true
	fHoopSpacing=1.5
	HoopTrailLen=2
	InitialHoopTrailEnd=0
	bHoopsVisible=true
	bTrackProgress=false
}
