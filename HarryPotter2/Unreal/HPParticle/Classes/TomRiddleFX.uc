//=============================================================================
// Particle to be attached to Tom Riddle
//=============================================================================
class TomRiddleFX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=1,Rand=1)
     SourceWidth=(Base=30,Rand=10)
     SourceHeight=(Base=30,Rand=10)
     SourceDepth=(Base=30,Rand=10)
     AngularSpreadWidth=(Base=15,Rand=10)
     AngularSpreadHeight=(Base=15,Rand=10)
     bSteadyState=True
     Speed=(Base=1,Rand=1)
     Lifetime=(Base=4,Rand=1)
     ColorStart=(Base=(R=0))
     ColorEnd=(Base=(R=0,G=128,B=64))
     AlphaStart=(Base=0.4)
     SizeWidth=(Base=2,Rand=4)
     SizeLength=(Base=2,Rand=4)
     SizeEndScale=(Base=2)
     SpinRate=(Base=0.2,Rand=2)
     SizeDelay=2
     Chaos=1.5
     ChaosDelay=0.5
     Gravity=(Z=5)
     ParticlesMax=300
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=-16352)
     DesiredRotation=(Pitch=-16352)
}
