//=============================================================================
// Aloh_hit.
//=============================================================================
class Aloh_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=30)
     SourceWidth=(Base=1)
     SourceHeight=(Base=64)
     SourceDepth=(Base=1)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=5)
     Lifetime=(Base=2,Rand=3)
     ColorStart=(Base=(R=253,G=152,B=0))
     ColorEnd=(Base=(G=202,B=40))
     SizeWidth=(Rand=12)
     SizeLength=(Rand=12)
     SizeEndScale=(Base=0,Rand=2)
     SpinRate=(Base=-1,Rand=2)
     ParticlesMax=60
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Physics=PHYS_Rotating
     bFixedRotationDir=True
     RotationRate=(Pitch=50000)
}
