//=============================================================================
// Pixie particles that emit from an exploding cornish pixie
//=============================================================================
class PixieExplode expands PixieParticles;

defaultproperties
{
     ParticlesPerSec=(Base=3000)
     SourceWidth=(Base=5)
     SourceHeight=(Base=15)
     SourceDepth=(Base=5)
     AngularSpreadWidth=(Base=90)
     AngularSpreadHeight=(Base=90)
     bSteadyState=True
     Speed=(Base=500,Rand=50)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(R=0,B=255))
     ColorEnd=(Base=(R=0,G=128,B=192))
     SizeWidth=(Base=20,Rand=15)
     SizeLength=(Base=20,Rand=15)
     SpinRate=(Base=-5,Rand=5)
     DripTime=(Base=0.1,Rand=0.1)
     Elasticity=0.5
     Damping=4
     GravityModifier=0.75
     ParticlesMax=200
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Location=(Z=100)
     Rotation=(Pitch=16408)
}
