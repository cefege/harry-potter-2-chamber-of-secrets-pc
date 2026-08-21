//=============================================================================
//=============================================================================
class BasilEyeSmoke expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=4)
    SourceWidth=(Base=4)
    SourceHeight=(Base=0)
    SourceDepth=(Base=4)
    AngularSpreadWidth=(Base=90)
    AngularSpreadHeight=(Base=90)
    Speed=(Rand=20)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(R=126,G=126,B=131))
    ColorEnd=(Base=(R=0))
    SizeWidth=(Base=10,Rand=5)
    SizeLength=(Base=10,Rand=5)
    SizeEndScale=(Base=3,Rand=1)
    SpinRate=(Base=-1,Rand=2)
    Chaos=2
    Damping=2
    //ParticlesMax=32
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke1'
    Rotation=(Pitch=16464)
}
