//=============================================================================
//  Particly Wind to emit from clouds for Flying Ford Level HP2 COS version.
//=============================================================================
class CloudWind expands FlyingFord;

defaultproperties
{
    ParticlesPerSec=(Base=4)
    SourceWidth=(Base=30)
    SourceHeight=(Base=100)
    bSteadyState=True
    Speed=(Base=300,Rand=150)
    Lifetime=(Base=2)
    ColorStart=(Base=(R=128,B=255))
    ColorEnd=(Base=(R=0,B=255))
    SizeWidth=(Base=20)
    SizeLength=(Base=20)
    SizeEndScale=(Base=2)
    SpinRate=(Base=3,Rand=2)
    Textures(0)=Texture'HPParticle.hp_fx.Particles.swirl001'
    LastUpdateLocation=(X=-104.6667,Y=5.333333,Z=-74)
    LastEmitLocation=(X=-104.6667,Y=5.333333,Z=-74)
    EmissionResidue=0.2820904
    Age=10880.93
    ParticlesEmitted=117183
    Level=LevelInfo'MyLevel.LevelInfo0'
    Tag=ParticleFX
    Region=(Zone=LevelInfo'MyLevel.LevelInfo0',iLeaf=1,ZoneNumber=1)
    Location=(X=-104.6667,Y=5.333333,Z=-74)
    Rotation=(Pitch=0)
    OldLocation=(X=-122,Y=16,Z=-16)
}
