//=============================================================================
// Hork03, this particle fx is when the Horkalump explodes and produces a Gas cloud.
//=============================================================================
class Hork03 expands horklumpsfx;

defaultproperties
{
    ParticlesPerSec=(Base=3000)
    SourceWidth=(Rand=4)
    SourceHeight=(Rand=4)
    SourceDepth=(Base=10,Rand=4)
    AngularSpreadWidth=(Base=75,Rand=30)
    AngularSpreadHeight=(Base=75,Rand=30)
    Speed=(Base=90,Rand=20)
    Lifetime=(Base=6,Rand=4)
    ColorStart=(Base=(R=155,G=0,B=249))
    ColorEnd=(Base=(R=97,G=52,B=103))
    SizeWidth=(Base=6,Rand=4)
    SizeLength=(Base=6,Rand=4)
    SizeEndScale=(Base=0.01,Rand=10)
    SpinRate=(Base=-4,Rand=8)
    Chaos=2
    ChaosDelay=1
    Damping=3
    ParticlesMax=80
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=16323)
}
