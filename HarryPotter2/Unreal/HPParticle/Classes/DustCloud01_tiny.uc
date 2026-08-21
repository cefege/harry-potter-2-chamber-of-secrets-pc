//=============================================================================
// tiny dust cloud
//=============================================================================
class DustCloud01_tiny expands Dustclouds;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=16)
    SourceHeight=(Base=16)
    AngularSpreadWidth=(Base=45)
    AngularSpreadHeight=(Base=45)
    Speed=(Base=20)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(R=142,G=112,B=87),Rand=(R=176,G=138,B=87))
    ColorEnd=(Base=(R=141,G=121,B=114),Rand=(R=168,G=139,B=81))
    SizeWidth=(Rand=6)
    SizeLength=(Rand=6)
    SizeEndScale=(Base=2,Rand=1)
    SpinRate=(Base=-1,Rand=2)
    Chaos=1
    Damping=0.5
    ParticlesMax=20
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=48995)
}
