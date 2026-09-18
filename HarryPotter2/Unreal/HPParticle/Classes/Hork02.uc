//=============================================================================
// Hork02, this particle fx is what happens when Harry shoots a Horkalump.
//=============================================================================
class Hork02 expands horklumpsfx;

defaultproperties
{
    ParticlesPerSec=(Base=1000)
    SourceWidth=(Rand=4)
    SourceHeight=(Rand=4)
    SourceDepth=(Base=10,Rand=4)
    AngularSpreadWidth=(Base=75,Rand=30)
    AngularSpreadHeight=(Base=75,Rand=30)
    Speed=(Rand=20)
    Lifetime=(Base=2,Rand=4)
    ColorStart=(Base=(R=176,G=89,B=240))
    ColorEnd=(Base=(R=97,G=52,B=103))
    SizeWidth=(Base=6,Rand=4)
    SizeLength=(Base=6,Rand=4)
    SizeEndScale=(Base=0.01,Rand=10)
    SpinRate=(Base=-4,Rand=8)
    Chaos=2
    ChaosDelay=1
    Damping=2
    GravityModifier=-0.05
    ParticlesMax=80
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=48995)
}
