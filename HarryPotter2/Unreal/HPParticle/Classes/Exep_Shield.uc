//=============================================================================
// Shield spell fx for Wizard dueling
//=============================================================================
class Exep_Shield expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=1000)
    SourceWidth=(Base=40,Rand=40)
    SourceHeight=(Base=40,Rand=40)
    bSteadyState=True
    Speed=(Base=0)
    Lifetime=(Base=0.25,Rand=0.5)
    ColorStart=(Base=(R=252,G=165,B=46))
    ColorEnd=(Base=(G=156,B=21))
    SizeWidth=(Base=6,Rand=12)
    SizeLength=(Base=6,Rand=12)
    SpinRate=(Base=-4,Rand=8)
    Chaos=5
    ParticlesMax=200
    Textures(0)=Texture'HPParticle.hp_fx.Particles.FF_Wind'
}
