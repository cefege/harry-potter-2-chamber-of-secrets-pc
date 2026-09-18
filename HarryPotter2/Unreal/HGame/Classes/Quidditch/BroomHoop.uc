//=============================================================================
// BroomHoop -- A hoop that Harry can fly a broom through
//=============================================================================
class BroomHoop extends HProp;

var(BroomHoop) int		PathNumber;			// Which set of stages is the hoop part of (one-based)
var(BroomHoop) int		Stage;				// Which stage is hoop part of (one-based)
var(BroomHoop) int		HoopNumber;			// Where hoop is in sequence of hoops within stage (one-based)

var(BroomHoop) float	PlayScale;			// The DrawScale the hoop should be in when it is in play
var(BroomHoop) bool		bBobbing;			// Whether hoop should bob up and down when it is in play

var float	fOriginalParticlesPerSec;		// What the set rate is for a full sized hoop
const		fSmallParticlesPerSec = 50.0f;	// What the particle rate is for a small, standby sized hoop

var bool	bPulseUp;
var float	fGlowOutRate;

//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
	Super.PostBeginPlay();

	// Don't draw the mesh during game, let the particles be the hoop.
	// (The DrawType is DT_Mesh in the editor so the hoop can be seen there)
	DrawType=DT_None;

	// All hoop start out invisible
	InitialState = 'HoopInvisible';
//	if ( bBobbing && !bDoBob )			/***/
//		startBobbing();

	// Capture the original particle rate
	if ( attachedParticleFX[0] != None )
		fOriginalParticlesPerSec = attachedParticleFX[0].ParticlesPerSec.Base;
}

function changeAttachedParticleFX( class<ParticleFX> newFX )
{
	if( attachedParticleFX[0] != None )
		attachedParticleFX[0].destroy();

	attachedParticleFX[0]=spawn( newFX,self, , location+attachedParticleOffset[0] );

	if ( attachedParticleFX[0] != None )
	{
		attachedParticleFX[0].setRotation( newFX.default.rotation );
		attachedParticleFX[0].setPhysics( PHYS_Trailer );

		fOriginalParticlesPerSec = attachedParticleFX[0].ParticlesPerSec.Base;

		if ( IsInState( 'HoopVisible' ) || IsInState( 'HoopAppearing' ) )
			attachedParticleFX[0].ParticlesPerSec.Base = fSmallParticlesPerSec;
	}
}

//-------------------------------------------------------------------------------------------
// States
//
// HoopInvisible	- Hoop is not in play right now
// HoopAppearing	- Hoop is becoming visible by growing up to stand-by size
// HoopVisible		- Hoop is visible and at stand-by size, but not the one to hit yet
// HoopNextToHit	- Hoop is visible and at full size to indicate that it is next to hit
// HoopDisappearing	- Hoop is becoming invisible by growing more and thinning out to nothing
//-------------------------------------------------------------------------------------------

state HoopInvisible
{
	// Just stays hidden
	function BeginState()
	{
//		bHidden = true;
		if (attachedParticleFX[0] != none)
			attachedParticleFX[0].EnableEmission(false);
		PlayAnim( 'Hold1', , 0.0 );
	}
}


state HoopAppearing
{
	// Appear in stand-by size
	function BeginState()
	{
		bHidden = false;
		if (attachedParticleFX[0] != none)
		{
			attachedParticleFX[0].EnableEmission(true);
			attachedParticleFX[0].ParticlesPerSec.Base = fSmallParticlesPerSec;
		}
		DrawScale = PlayScale;
		ScaleGlow = 1.0;
		PlayAnim( 'Hold1', , 0.0 );

//		if ( bBobbing && !bDoBob )		/***/
//			startBobbing();
	}

	event AnimEnd()
	{
		GotoState( 'HoopVisible' );
		LoopAnim( 'Hold1' );
	}
}


state HoopVisible
{
	// Stay in stand-by size and visible
	function BeginState()
	{
		bHidden = false;
		if (attachedParticleFX[0] != none)
		{
			attachedParticleFX[0].EnableEmission(true);
			attachedParticleFX[0].ParticlesPerSec.Base = fSmallParticlesPerSec;
		}
		DrawScale = PlayScale;
		ScaleGlow = 1.0;
//		if ( bBobbing && !bDoBob )		/***/
//			startBobbing();
	}
}


state HoopNextToHit
{
	// Grow to full size
	function BeginState()
	{
		bHidden = false;
		if (attachedParticleFX[0] != none)
		{
			attachedParticleFX[0].EnableEmission(true);
			attachedParticleFX[0].ParticlesPerSec.Base = fOriginalParticlesPerSec;
		}
		DrawScale = PlayScale;
		ScaleGlow = 1.0;
		PlayAnim( 'Grow2', , 0.0 );
		bPulseUp = true;
//		if ( bBobbing && !bDoBob )		/***/
//			startBobbing();
	}

	event AnimEnd()
	{
		LoopAnim( 'Hold3' );
	}

	event Tick( float DeltaTime )
	{
		Super.Tick( DeltaTime );

		if ( bPulseUp )
		{
			ScaleGlow += 20.0 * DeltaTime;
			if ( ScaleGlow >= 3.0 )
			{
				ScaleGlow = 5.2;
				bPulseUp = false;
			}
		}
		else
		{
			ScaleGlow -= 20.0 * DeltaTime;
			if ( ScaleGlow < 0.2 )
			{
				ScaleGlow = 0.2;
				bPulseUp = true;
			}
		}
	}
}


state HoopDisappearing
{
	// Grow more until vaporized
	function BeginState()
	{
		bHidden = false;
		if (attachedParticleFX[0] != none)
		{
			attachedParticleFX[0].EnableEmission(true);
			attachedParticleFX[0].ParticlesPerSec.Base = fOriginalParticlesPerSec;
		}
		PlayAnim( 'Die' );
		fGlowOutRate = ScaleGlow / 0.7;	// Amount of glow to reduce per second and take 0.7 seconds doing it
	}

	event AnimEnd()
	{
//		GotoState( 'HoopInvisible' );
	}

	event Tick( float DeltaTime )
	{
		Super.Tick( DeltaTime );
		ScaleGlow -= fGlowOutRate * DeltaTime;
		if ( ScaleGlow <= 0.0 )
		{
			ScaleGlow = 0.0;
			GotoState( 'HoopInvisible' );
		}
	}
}


defaultproperties
{
	Mesh=SkeletalMesh'HProps.skBroomHoopMesh'
	DrawType=DT_Mesh
	bStatic=false
	bDirectional=true
	PathNumber=1
	HoopNumber=1
	PlayScale=1.0

	CollisionHeight=70.0
	CollisionRadius=70.0
	bCollideActors=false
	bCollideWorld=false
	bBlockActors=false
	bBlockPlayers=false

	bBobbing=false
//	fBobAmount=24.0
}

