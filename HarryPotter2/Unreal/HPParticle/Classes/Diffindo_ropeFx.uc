//=============================================================================
// FX for diffindo rope
//=============================================================================
class Diffindo_ropeFx expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=50,Rand=10)
    SourceWidth=(Base=64)
    SourceHeight=(Base=5)
    AngularSpreadWidth=(Base=0)
    AngularSpreadHeight=(Base=0)
    bSteadyState=True
    Speed=(Base=-15,Rand=30)
    Lifetime=(Base=1.5,Rand=0.75)
    ColorStart=(Base=(G=243,B=104),Rand=(R=255))
    ColorEnd=(Base=(G=255),Rand=(R=255,G=145,B=53))
    SizeWidth=(Base=6,Rand=4)
    SizeLength=(Base=6,Rand=4)
    SizeEndScale=(Base=0,Rand=2)
    SpinRate=(Base=-2,Rand=4)
    DripTime=(Base=0.5)
    Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
    Rotation=(Pitch=48995)
}
