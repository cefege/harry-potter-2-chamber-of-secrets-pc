//=============================================================================
// RotatingMover.
//=============================================================================

class RotatingMover extends Mover;

var() bool	  bUseRotateStep;
var() rotator RotateStep;

function BeginPlay()
{
	Super.BeginPlay();
	
	Disable( 'Tick' );
}


function Tick( float fTimeDelta )
{
	Super.Tick( fTimeDelta );

	// If we are not using RotateSteps then we will not rely on DesiredRotation
	if(!bUseRotateStep)
	{	
		SetRotation( Rotation + (RotationRate * fTimeDelta) );
	}
}

// Open the mover. (copied from parent but removed interpolateTo() )
function DoOpen()
{
	bOpening  = true;
	bDelaying = false;
//	InterpolateTo( 1, MoveTime );
	PlaySound(OpeningSound, SLOT_None, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0);
	PlaySound(MoveAmbientSound, SLOT_Misc, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0, , true);
//	AmbientSound = MoveAmbientSound;
	
	if(bUseRotateStep)
	{
		DesiredRotation  += RotateStep;
		log( "RotatingMover::DoOpen() -> Current rot = " $rotation $" desiredRot = " $DesiredRotation );
	}
	
}

// Close the mover.(copied from parent but removed interpolateTo() )
function DoClose()
{
	local actor A;

	bOpening  = false;
	bDelaying = false;
//	InterpolateTo( Max(0,KeyNum-1), MoveTime );
	PlaySound(ClosingSound, SLOT_None, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0);
	if( Event != '' )
		foreach AllActors( class 'Actor', A, Event )
			A.UnTrigger( Self, Instigator );
	PlaySound(MoveAmbientSound, SLOT_Misc, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0, , true);
//	AmbientSound = MoveAmbientSound;

	Disable('Tick');
}

defaultproperties
{
	bRotateToDesired=true;
}
