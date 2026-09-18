//=============================================================================
// Hork01, this particle fx is for when Harry walks into a horkalump.
//=============================================================================
class Hork01 expands horklumpsfx;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Rand=4)
    SourceHeight=(Rand=4)
    SourceDepth=(Base=10,Rand=4)
    AngularSpreadWidth=(Base=75,Rand=30)
    AngularSpreadHeight=(Base=75,Rand=30)
    bSteadyState=True
    Speed=(Rand=20)
    Lifetime=(Base=1.5,Rand=3)
    ColorStart=(Base=(R=192,G=60,B=200))
    ColorEnd=(Base=(R=97,G=52,B=103))
    SizeWidth=(Base=6,Rand=4)
    SizeLength=(Base=6,Rand=4)
    SizeEndScale=(Base=0.01,Rand=10)
    SpinRate=(Base=-1.5,Rand=3)
    Chaos=2
    ChaosDelay=1
    Damping=2
    GravityModifier=-0.05
    ParticlesMax=25
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=48995)
}
