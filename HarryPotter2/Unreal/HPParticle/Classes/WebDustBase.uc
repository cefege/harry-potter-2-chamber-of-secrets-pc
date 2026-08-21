//=============================================================================
// fx for Aragog Support web, dust/smoke 
//=============================================================================
class WebDustBase expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=30,Rand=10)
    SourceHeight=(Base=175,Rand=10)
    SourceDepth=(Base=30,Rand=10)
    AngularSpreadWidth=(Base=90)
    AngularSpreadHeight=(Base=90)
    Speed=(Rand=20)
    Lifetime=(Base=3,Rand=1)
    ColorStart=(Base=(R=126,G=126,B=131))
    ColorEnd=(Base=(R=0))
    SizeWidth=(Base=16,Rand=10)
    SizeLength=(Base=16,Rand=10)
    SizeEndScale=(Base=3,Rand=1)
    SpinRate=(Base=-1,Rand=2)
    Chaos=2
    Damping=2
    ParticlesMax=64
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke1'
    Rotation=(Pitch=16464)
}
