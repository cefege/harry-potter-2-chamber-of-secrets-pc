//=============================================================================
// SpellLearnFX.
//=============================================================================
class SpellLearnFX expands actor;
//class SpellLearnFX expands ParticleFX;

// Settings.
var Gesture	SpellGesture;
var float TotalTime;
var float FXSize;
var float fFXDecayTime;

// Temp vars.
var vector OrigLoc;
var int I;

var ParticleFX			Sparkle;

var int	iCounter;
var	float	TimerToPoint;

var() bool	bStarted;

event Destroyed()
{
	// Done.
	Sparkle.Destroy();
	Super.Destroyed();
}

function SetupFX(int iType)
{
/*	switch (iType)
	{
		case 0:
			Sparkle = Spawn( class'BronzeSparkle01', [SpawnLocation] Location);
			break;

		case 1:
			Sparkle = Spawn( class'SilverSparkle01', [SpawnLocation] Location);
			break;

		case 2:
			Sparkle = Spawn( class'GoldSparkle01', [SpawnLocation] Location);
			break;
	}
*/
	Sparkle = Spawn( class'SilverSparkle01', [SpawnLocation] Location);

//	Sparkle.Lifetime.base = TotalTime * fFXDecayTime * 2;
	Sparkle.Lifetime.base = 99999999;
	Sparkle.Lifetime.Rand = 0;

    Sparkle.SizeWidth.Base = 3.0 * FXSize;
    Sparkle.SizeLength.Base = 3.0 * FXSize;
}

function DrawSpell( Gesture gesture, float inSize, float inTime, int iType, float fAccuracy, float fDecayTime)
{
	if( int(gesture.Points) < 2 )
		return;
	SpellGesture = gesture;
	OrigLoc = Location;
	TotalTime = inTime;
	FXSize = fAccuracy / 0.01;
	fFXDecayTime = fDecayTime;

	SetupFX(iType);
	Sparkle.Pattern = gesture;
	Sparkle.DrawScale = inSize;
	Sparkle.Period.Base = 0;
	Sparkle.Period.Rand = 0;

//	PlaySound(sound'spell_example_loop', SLOT_Interact);

	gotoState('DrawState');
}

state DrawState
{
	function Tick( float DeltaTime )
	{
		Sparkle.Period.Base += Sparkle.Period.Rand;
		if( Sparkle.Period.Base > 1.0 )
		{
			Sparkle.Period.Rand = 0;
//			PlaySound(sound'spell_example_loop', SLOT_Interact, 0,,0); //kill the sound!!!!
			return;
		}
		Sparkle.Period.Rand = DeltaTime / TotalTime;
//		log( Sparkle$ " Tick " $DeltaTime$ " Base=" $Sparkle.Period.Base$ " Rand=" $Sparkle.Period.Rand );
	}
}

defaultproperties
{
	DrawType=DT_None
	bStarted=false
}
/*{
     Distribution=DIST_Uniform
     ParticlesPerSec=(Base=2.000000)
     SourceWidth=(Base=0.000000)
     SourceHeight=(Base=0.000000)
     AngularSpreadWidth=(Base=0.000000)
     AngularSpreadHeight=(Base=0.000000)
     speed=(Base=0.000000)
     Lifetime=(Base=15.000000)
     ColorStart=(Base=(R=0,G=255))
     ColorEnd=(Base=(R=64,G=128,B=128))
     SizeEndScale=(Base=2)
}*/
/*
	if (pos.Y > 0.5 || pos.Y < -0.5 || pos.X > 0.5 || pos.X < -0.5)
	{
		// bad data value, set the point to be the previous point
		pos.Y = SpellGesture.Points[i - 1].X - 0.5;
		pos.Z = 0.5 - SpellGesture.Points[i - 1].Y;
	}
*/