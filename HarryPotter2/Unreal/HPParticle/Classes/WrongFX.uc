//=============================================================================
// Wrongfx.
//=============================================================================
class Wrongfx expands ParticleFX;

defaultproperties
{
      ParticlesPerSec=(Base=500)
    SourceWidth=(Base=48)
    SourceHeight=(Base=48)
    SourceDepth=(Base=24)
    bSteadyState=True
    Speed=(Base=5,Rand=25)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(R=254,G=192,B=101))
    ColorEnd=(Base=(R=128,G=119,B=136))
    SizeWidth=(Base=12,Rand=4)
    SizeLength=(Base=12,Rand=4)
    SizeEndScale=(Base=-1,Rand=4)
    SpinRate=(Base=-2,Rand=2)
    Chaos=10
    ChaosDelay=0.25
    ParticlesMax=50
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
     Rotation=(Pitch=-16840,Yaw=7776)
     bRotateToDesired=True
}
