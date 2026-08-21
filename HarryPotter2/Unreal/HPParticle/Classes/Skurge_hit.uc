//=============================================================================
// Skurge_hit.
//=============================================================================
class Skurge_hit expands AllSpellCast_FX;

function PreBeginPlay()
{
	Super.PreBeginPlay();
	SetRotation( DesiredRotation );// set our rotation to be our desiredRotation
}

defaultproperties
{
	DesiredRotation=(Yaw=0,Pitch=16464,Roll=0)
	ParticlesPerSec=(Base=50)
	SourceWidth=(Base=64)
	SourceHeight=(Base=64)
	SourceDepth=(Base=10)
	AngularSpreadWidth=(Base=60)
	AngularSpreadHeight=(Base=60)
	bSteadyState=True
	Speed=(Rand=80)
	Lifetime=(Base=3,Rand=3)
	ColorStart=(Base=(R=94,G=225,B=97))
	ColorEnd=(Base=(R=99,G=172,B=68))
	SizeWidth=(Base=12,Rand=6)
	SizeLength=(Base=12,Rand=6)
	SizeEndScale=(Base=-1,Rand=2)
	Chaos=1
	Elasticity=0.1
	Damping=1
	GravityModifier=1
	ParticlesMax=0
	Textures(0)=Texture'HPParticle.hp_fx.Particles.blob32'
	Rotation=(Pitch=16352)
	bFixedRotationDir=True
	bRotateToDesired=True
}
