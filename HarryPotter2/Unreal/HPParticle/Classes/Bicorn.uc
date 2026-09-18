//=============================================================================
// bicorn horn particle fx
//=============================================================================
class Bicorn expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=20,Rand=10)
    SourceWidth=(Base=30,Rand=5)
    SourceHeight=(Base=30,Rand=5)
    SourceDepth=(Base=20,Rand=10)
    bSteadyState=True
    Speed=(Base=10,Rand=5)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(R=78,G=65,B=241),Rand=(R=117,B=247))
    ColorEnd=(Base=(R=100,G=175,B=67),Rand=(R=55,G=247,B=34))
    SizeWidth=(Base=2,Rand=2)
    SizeLength=(Base=6,Rand=12)
    SpinRate=(Base=-4,Rand=8)
    Chaos=2
    Attraction=(X=5,Y=5)
    Damping=0.5
    Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
    Rotation=(Pitch=16464)
}
