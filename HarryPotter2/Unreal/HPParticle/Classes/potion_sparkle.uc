//=============================================================================
// Potion_sparkle.
//=============================================================================
class Potion_sparkle expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=80)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=135)
     AngularSpreadHeight=(Base=135)
     Speed=(Base=40)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=0),Rand=(R=13,B=249))
     SizeWidth=(Base=1,Rand=6)
     SizeLength=(Base=1,Rand=6)
     SizeEndScale=(Base=0,Rand=3)
     SpinRate=(Base=-6,Rand=12)
     SizeDelay=0.3
     Chaos=1
     Attraction=(X=20,Y=20)
     Damping=1
     GravityModifier=-0.1
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_BlueSmoke'
     LastUpdateLocation=(X=-0.277706,Y=224,Z=-16.59853)
     LastEmitLocation=(X=-0.277706,Y=224,Z=-16.59853)
     LastUpdateRotation=(Pitch=16504)
     EmissionResidue=0.2353935
     Age=1615.908
     ParticlesEmitted=129563
     bDynamicLight=True
     Tag=Dummyparticle
     Location=(X=-0.277706,Y=224,Z=-16.59853)
     Rotation=(Pitch=-16352)
     DesiredRotation=(Pitch=-16352)
}
