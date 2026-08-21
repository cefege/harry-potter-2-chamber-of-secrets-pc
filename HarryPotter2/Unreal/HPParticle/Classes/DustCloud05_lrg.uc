//=============================================================================
// Large version dust cloud
//=============================================================================
class DustCloud05_lrg expands Dustclouds;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=128)
    SourceHeight=(Base=128)
    AngularSpreadWidth=(Base=45)
    AngularSpreadHeight=(Base=45)
    Speed=(Base=120)
    Lifetime=(Base=2,Rand=3)
    ColorStart=(Base=(R=139,G=118,B=99),Rand=(R=139,G=134,B=90))
    ColorEnd=(Base=(R=141,G=121,B=114),Rand=(R=154,G=123,B=92))
    SizeWidth=(Base=64,Rand=24)
    SizeLength=(Base=64,Rand=24)
    SizeEndScale=(Base=2,Rand=1)
    SpinRate=(Base=-1,Rand=2)
    Chaos=1
    Damping=1.25
    ParticlesMax=25
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=48995)
}
