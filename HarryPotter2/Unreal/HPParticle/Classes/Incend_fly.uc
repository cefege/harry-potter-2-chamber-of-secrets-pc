//=============================================================================
// Incend_fly.
//=============================================================================
class Incend_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=80)
     SourceWidth=(Base=2)
     SourceHeight=(Base=2)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     Speed=(Base=25)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=30,B=255))
     SizeWidth=(Base=3,Rand=6)
     SizeLength=(Base=3,Rand=6)
     SizeEndScale=(Base=5,Rand=8)
     SpinRate=(Base=-6,Rand=12)
     Attraction=(X=5,Y=5,Z=5)
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_BlueSmoke'
     LastUpdateLocation=(Y=20,Z=32)
     LastEmitLocation=(Y=20,Z=32)
     EmissionResidue=0.06977463
     Age=434.0083
     ParticlesEmitted=88822
     Tag=Flip_fly
     Location=(Y=20,Z=32)
     Rotation=(Pitch=0)
}
