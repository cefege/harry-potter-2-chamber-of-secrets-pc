//=============================================================================
// Pixie particles that emit from flying cornish pixie
//=============================================================================
class PixieFlying expands PixieParticles;

defaultproperties
{
     ParticlesPerSec=(Base=50)
     SourceWidth=(Base=5)
     SourceDepth=(Base=10)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=0)
     Lifetime=(Base=0.75,Rand=0.5)
     ColorStart=(Base=(R=0,G=128,B=255))
     ColorEnd=(Base=(R=0,G=128,B=192))
     SizeWidth=(Base=6,Rand=2)
     SizeLength=(Base=6,Rand=2)
     SpinRate=(Base=-2,Rand=4)
     GravityModifier=0.05
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Rotation=(Pitch=16323)
}
