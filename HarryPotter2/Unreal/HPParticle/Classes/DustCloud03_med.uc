//=============================================================================
// Medium version A dust cloud
//=============================================================================
class DustCloud03_med expands Dustclouds;

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
    ColorStart=(Base=(R=146,G=115,B=95),Rand=(R=157,G=121,B=94))
    ColorEnd=(Base=(R=141,G=121,B=114),Rand=(R=159,G=119,B=89))
    SizeWidth=(Base=32,Rand=16)
    SizeLength=(Base=32,Rand=16)
    SizeEndScale=(Base=-1,Rand=3)
    SpinRate=(Base=-1,Rand=2)
    Chaos=1
    Damping=0.75
    ParticlesMax=50
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=16464)
}
