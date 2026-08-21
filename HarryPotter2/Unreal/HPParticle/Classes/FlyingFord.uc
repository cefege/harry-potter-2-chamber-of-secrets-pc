//=============================================================================
//  Streamy Wind for Flying Ford Level HP2 COS version.
//=============================================================================
class FlyingFord expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Base=4)
    SourceWidth=(Base=30)
    SourceHeight=(Base=50)
    bSteadyState=True
    Speed=(Base=250,Rand=200)
    Lifetime=(Base=2)
    ColorStart=(Base=(R=128,B=128))
    ColorEnd=(Base=(R=0,B=255))
    SizeWidth=(Base=60)
    SizeLength=(Base=30)
    SizeEndScale=(Base=2)
    Textures(0)=Texture'HPParticle.hp_fx.FF_Wind'
    LastUpdateLocation=(X=-128.0759,Y=-40.05203,Z=185.9999)
    LastEmitLocation=(X=-128.0759,Y=-40.05203,Z=185.9999)
    LastUpdateRotation=(Pitch=16)
    EmissionResidue=0.3380978
    Age=12313.84
    ParticlesEmitted=117612
    Level=LevelInfo'MyLevel.LevelInfo0'
    Tag=ParticleFX
    Region=(Zone=LevelInfo'MyLevel.LevelInfo0',ZoneNumber=1)
    Location=(X=-128.0759,Y=-40.05203,Z=185.9999)
    Rotation=(Pitch=16)
    OldLocation=(X=32,Y=-16,Z=-16)
}
