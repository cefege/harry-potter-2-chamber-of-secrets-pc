//=============================================================================
// Petri_fly.
//=============================================================================
class Petri_fly expands AllSpellCast_FX;

defaultproperties
{
    ParticlesPerSec=(Base=25.000000)
    SourceWidth=(Base=5.000000)
    SourceHeight=(Base=5.000000)
    Period=(Base=5.000000)
    bSteadyState=True
    speed=(Base=100.000000)
    Lifetime=(Base=4.000000)
    ColorStart=(Base=(R=128,B=128))
    ColorEnd=(Base=(R=0))
    SizeWidth=(Base=15.000000,Rand=4.000000)
    SizeLength=(Base=15.000000,Rand=4.000000)
    SizeEndScale=(Base=5.000000)
    SpinRate=(Base=9.000000)
    DripTime=(Base=0.250000)
    Damping=1.000000
    Textures(0)=FireTexture'HPParticle.hp_fx.Spells.WIN_P'
    LastUpdateLocation=(X=-383.860046,Y=-190.726883,Z=65.730934)
    LastEmitLocation=(X=-383.860046,Y=-190.726883,Z=65.730934)
    LastUpdateRotation=(Pitch=16144,Yaw=-16336)
    EmissionResidue=0.247931
    Age=19861.437500
    CurrentPriorityTag=2
    bDynamicLight=True
    Level=LevelInfo'MyLevel.LevelInfo0'
    Tag=ParticleFX
    Region=(Zone=LevelInfo'MyLevel.LevelInfo0',ZoneNumber=1)
    Location=(X=-383.860046,Y=-190.726883,Z=65.730934)
    Rotation=(Pitch=16144,Yaw=-16336)
    OldLocation=(X=1.133467,Y=115.525375,Z=68.572403)
    Name=ParticleFX2
}
