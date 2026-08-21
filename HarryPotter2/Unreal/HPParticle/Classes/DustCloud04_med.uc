//=============================================================================
// Medium version B dust cloud
//=============================================================================
class DustCloud04_med expands Dustclouds;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=128)
    SourceHeight=(Base=128)
    SourceDepth=(Base=128)
    AngularSpreadWidth=(Base=45)
    AngularSpreadHeight=(Base=45)
    Speed=(Base=80)
    Lifetime=(Base=2,Rand=3)
    ColorStart=(Base=(R=148,G=106,B=82),Rand=(R=143,G=131,B=92))
    ColorEnd=(Base=(R=141,G=121,B=114),Rand=(R=147,G=118,B=79))
    SizeWidth=(Base=32,Rand=16)
    SizeLength=(Base=32,Rand=16)
    SizeEndScale=(Base=2,Rand=1)
    SpinRate=(Base=-1,Rand=2)
    Chaos=1
    Damping=0.75
    ParticlesMax=50
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=16464)
}
