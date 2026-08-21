//=============================================================================
// Flip_wand.
//=============================================================================
class Flip_wand expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Base=200.000000)
    SourceWidth=(Base=2.000000)
    SourceHeight=(Base=2.000000)
    AngularSpreadWidth=(Base=10.000000,Rand=2.000000)
    AngularSpreadHeight=(Base=10.000000,Rand=2.000000)
    bSteadyState=True
    ColorEnd=(Base=(R=30,G=30,B=30))
    SizeWidth=(Base=3.000000)
    SizeLength=(Base=3.000000)
    SizeEndScale=(Base=5.000000)
    SpinRate=(Base=-3.000000)
    DripTime=(Base=0.200000)
    Attraction=(X=-50.000000,Z=-50.000000)
    Damping=1.500000
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_3'
    LastUpdateLocation=(X=-386.995544,Y=-383.679016,Z=68.572403)
    LastEmitLocation=(X=-386.995544,Y=-383.679016,Z=68.572403)
    LastUpdateRotation=(Yaw=-16336)
    EmissionResidue=0.178528
    Age=14217.859375
    CurrentPriorityTag=6
    Level=LevelInfo'MyLevel.LevelInfo0'
    Tag=ParticleFX
    Region=(Zone=LevelInfo'MyLevel.LevelInfo0',ZoneNumber=1)
    Location=(X=-386.995544,Y=-383.679016,Z=68.572403)
    Rotation=(Pitch=0,Yaw=-16336)
    OldLocation=(X=-32.000000,Y=64.000000,Z=96.000000)
    Name=ParticleFX1
}
