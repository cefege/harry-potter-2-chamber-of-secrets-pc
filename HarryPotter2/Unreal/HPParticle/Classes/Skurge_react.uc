//=============================================================================
// Skurge_React.
//=============================================================================
class Skurge_react expands AllSpellCast_FX;

function PreBeginPlay()
{
	Super.PreBeginPlay();
	SetRotation( DesiredRotation );// set our rotation to be our desiredRotation
}

defaultproperties
{
	DesiredRotation=(Yaw=0,Pitch=16464,Roll=0)

	ParticlesPerSec=(Base=50)
	SourceWidth=(Base=64,Rand=25)
	SourceHeight=(Base=64,Rand=25)
	SourceDepth=(Base=10)
	bSteadyState=True
	Speed=(Rand=40)
	Lifetime=(Base=2,Rand=3)
	ColorStart=(Base=(R=94,G=225,B=97))
	ColorEnd=(Base=(R=28,G=49,B=19))
	SizeWidth=(Base=15,Rand=15)
	SizeLength=(Base=15,Rand=15)
	SizeEndScale=(Base=-4,Rand=8)
	SpinRate=(Base=-2,Rand=4)
	Chaos=2
	Damping=0.5
	GravityModifier=0.05
	ParticlesMax=0
	Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
	Rotation=(Pitch=16352)
	bFixedRotationDir=True
	bRotateToDesired=True
}
