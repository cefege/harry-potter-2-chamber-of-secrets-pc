//=============================================================================
// Incend_hit.
//=============================================================================
class Incend_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=500)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=41,G=4,B=255))
     SizeWidth=(Base=15,Rand=5)
     SizeLength=(Base=15,Rand=5)
     SizeEndScale=(Base=0,Rand=4)
     SpinRate=(Base=-6,Rand=12)
     Chaos=1
     Attraction=(X=-5,Y=-5)
     Damping=10
     GravityModifier=-0.5
     ParticlesMax=100
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_BlueSmoke'
     LastUpdateLocation=(X=-383.86,Y=-178.5758,Z=65.73093)
     LastEmitLocation=(X=-383.86,Y=-178.5758,Z=65.73093)
     LastUpdateRotation=(Pitch=16144,Yaw=-16336)
     EmissionResidue=0.810257
     Age=13676.87
     Tag=ParticleFX
     Location=(X=-383.86,Y=-178.5758,Z=65.73093)
     Rotation=(Pitch=16144,Yaw=-16336)
     OldLocation=(X=1.133467,Y=115.5254,Z=68.5724)
}
