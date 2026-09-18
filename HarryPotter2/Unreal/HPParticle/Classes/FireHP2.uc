//=============================================================================
//  A torch fire fx for the HP COS version.
//=============================================================================
class FireHP2 expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Rand=10)
    SourceWidth=(Base=4,Rand=1)
    SourceHeight=(Base=4,Rand=1)
    Speed=(Base=10,Rand=15)
    Lifetime=(Rand=2)
    SizeWidth=(Base=12,Rand=4)
    SizeLength=(Base=12,Rand=4)
    SizeEndScale=(Base=-1,Rand=2)
    SpinRate=(Base=-2,Rand=4)
    Chaos=1
    ChaosDelay=1
    Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
    Rotation=(Pitch=16384)
}
