//=============================================================================
// Rictusempra hit spell fx
//=============================================================================
class Rictusempra_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=50000)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=500)
     ColorStart=(Base=(R=198,G=55,B=66))
     ColorEnd=(Base=(R=247,G=86))
     SizeWidth=(Base=32)
     SizeLength=(Base=32)
     SizeEndScale=(Base=0)
     SpinRate=(Base=1,Rand=2)
     bSystemRelative=True
     SizeDelay=0.25
     Damping=8
     ParticlesMax=100
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Physics=PHYS_Rotating
     bFixedRotationDir=True
     RotationRate=(Pitch=-40000)
}
