//=============================================================================
// Diffindo fly spell fx
//=============================================================================
class Diffindo_Fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=14)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=40)
     Lifetime=(Base=1.5)
     ColorStart=(Base=(R=121,G=255,B=11))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=16)
     SizeLength=(Base=16)
     SpinRate=(Base=1,Rand=8)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_03'
     Physics=PHYS_Rotating
     bFixedRotationDir=True
     RotationRate=(Pitch=50000)
}
