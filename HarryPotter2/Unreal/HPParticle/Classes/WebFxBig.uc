//=============================================================================
// fx for Aragog Arch Support web breakup
//=============================================================================
class WebFxBig expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=120,Rand=10)
    SourceHeight=(Base=175,Rand=10)
    SourceDepth=(Base=30,Rand=10)
    AngularSpreadWidth=(Base=90)
    AngularSpreadHeight=(Base=90)
    bSteadyState=True
    Speed=(Rand=20)
    Lifetime=(Base=2,Rand=2)
    ColorStart=(Base=(R=159,G=207,B=255))
    ColorEnd=(Base=(R=0))
    SizeWidth=(Base=10,Rand=8)
    SizeLength=(Base=1)
    SpinRate=(Base=-4,Rand=4)
    Chaos=6
    Damping=2
    ParticlesMax=100
    Textures(0)=Texture'HPParticle.hp_fx.Particles.webticle'
    Rotation=(Pitch=16464)
}
