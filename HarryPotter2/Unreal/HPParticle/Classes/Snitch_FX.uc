//=============================================================================
// Snitch_FX.
//=============================================================================
class Snitch_FX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=50)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     bSteadyState=True
     Speed=(Base=400)
     Lifetime=(Base=2,Rand=2)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=128,G=128,B=128),Rand=(R=128,G=128,B=128))
     SizeWidth=(Base=1,Rand=8)
     SizeLength=(Base=1,Rand=8)
     SizeEndScale=(Base=0,Rand=10)
     SpinRate=(Base=-2,Rand=4)
     AlphaDelay=1
     Chaos=1
     Attraction=(X=1,Y=1,Z=1)
     Damping=11
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_3'
     CollisionRadius=10000
}
