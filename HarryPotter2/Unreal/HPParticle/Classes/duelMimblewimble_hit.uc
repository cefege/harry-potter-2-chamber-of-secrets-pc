//=============================================================================
// duelMimblewimble hit spell fx
//=============================================================================
class duelMimblewimble_hit expands AllSpellCast_FX;

var Actor HitActor;

function Tick(float t)
{
	super.Tick(t);

	if(HitActor == none)
		return;

	// follow hit actor, till gone
	SetLocation(HitActor.location);
}

defaultproperties
{
     ParticlesPerSec=(Base=50000)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=500)
     ColorStart=(Base=(R=34,G=67,B=255))
     ColorEnd=(Base=(R=113,G=6,B=164))
     SizeWidth=(Base=15)
     SizeLength=(Base=15)
     SizeEndScale=(Base=0)
     SpinRate=(Base=1)
     bSystemRelative=True
     SizeDelay=0.25
     Damping=8
     ParticlesMax=100
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Physics=PHYS_Rotating
     bFixedRotationDir=True
     RotationRate=(Pitch=-40000)
}
