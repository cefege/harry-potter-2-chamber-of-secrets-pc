//=============================================================================
// Doxie_fx.
//=============================================================================
class Doxie_fx expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Base=20.000000)
    SourceWidth=(Base=2.000000)
    SourceHeight=(Base=2.000000)
    SourceDepth=(Base=20.000000)
    AngularSpreadWidth=(Base=180.000000)
    AngularSpreadHeight=(Base=180.000000)
    bSteadyState=True
    speed=(Base=20.000000,Rand=10.000000)
    Lifetime=(Base=2.000000,Rand=1.000000)
    ColorStart=(Base=(R=89,G=131,B=255))
    ColorEnd=(Base=(R=72,G=63,B=194))
    SizeWidth=(Base=6.000000,Rand=2.000000)
    SizeLength=(Base=6.000000,Rand=2.000000)
    SizeEndScale=(Base=0.000000)
    SpinRate=(Base=-4.000000,Rand=8.000000)
    Chaos=5.000000
    ChaosDelay=0.500000
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_5'
    Rotation=(Pitch=-16352)
    DesiredRotation=(Pitch=-16352)
}
