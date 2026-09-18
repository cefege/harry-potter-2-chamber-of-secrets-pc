//=============================================================================
// Pixie particles that emit from a hit cornish pixie
//=============================================================================
class PixieHit expands PixieParticles;

defaultproperties
{
     ParticlesPerSec=(Base=750)
     SourceDepth=(Base=10)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=300)
     Lifetime=(Base=0.7)
     ColorStart=(Base=(R=202,G=203,B=247))
     ColorEnd=(Base=(R=0,G=128,B=192))
     AlphaEnd=(Base=0.2)
     SizeWidth=(Base=10,Rand=5)
     SizeLength=(Base=10,Rand=5)
     SpinRate=(Base=-2,Rand=4)
     Attraction=(X=17,Y=17,Z=17)
     ParticlesMax=200
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Rotation=(Pitch=16323)
     CollisionRadius=25
     CollisionHeight=25
}
