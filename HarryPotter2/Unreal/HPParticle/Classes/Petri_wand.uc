//=============================================================================
// Petri_wand.
//=============================================================================
class Petri_wand expands AllSpellCast_FX;

defaultproperties
{
    ParticlesPerSec=(Base=20.000000)
    SourceWidth=(Base=1.000000)
    SourceHeight=(Base=1.000000)
    AngularSpreadWidth=(Base=0.000000)
    AngularSpreadHeight=(Base=0.000000)
    bSteadyState=True
    speed=(Base=10.000000)
    Lifetime=(Base=5.000000,Rand=2.000000)
    ColorStart=(Base=(R=128,B=128))
    ColorEnd=(Base=(R=30,G=30,B=30))
    SizeWidth=(Base=3.000000,Rand=1.000000)
    SizeLength=(Rand=2.000000)
    SizeEndScale=(Base=3.000000)
    DripTime=(Base=0.200000)
    Attraction=(X=35.000000,Y=35.000000)
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_5'
    LastUpdateLocation=(X=-388.530518,Y=-383.679016,Z=60.941666)
    LastEmitLocation=(X=-388.530518,Y=-383.679016,Z=60.941666)
    LastUpdateRotation=(Pitch=16528,Yaw=-16336)
    EmissionResidue=0.149399
    Age=20429.718750
    CurrentPriorityTag=5
    bDynamicLight=True
    Level=LevelInfo'MyLevel.LevelInfo0'
    Tag=ParticleFX
    Region=(Zone=LevelInfo'MyLevel.LevelInfo0',ZoneNumber=1)
    Location=(X=-388.530518,Y=-383.679016,Z=60.941666)
    Rotation=(Pitch=16528,Yaw=-16336)
    OldLocation=(X=-32.000000,Y=64.000000,Z=96.000000)
    Name=ParticleFX1
}
