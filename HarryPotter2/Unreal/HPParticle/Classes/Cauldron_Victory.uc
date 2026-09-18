//=============================================================================
// Cauldron_Victory.
//=============================================================================
class Cauldron_Victory expands CauldronFX;

defaultproperties
{
     ParticlesPerSec=(Rand=20.000000)
     SourceWidth=(Base=50.000000)
     SourceHeight=(Base=50.000000)
     speed=(Base=8.000000,Rand=35.000000)
     Lifetime=(Base=2.000000,Rand=8.000000)
     ColorStart=(Base=(R=230,G=239,B=255))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=5.000000,Rand=20.000000)
     SizeLength=(Base=5.000000,Rand=20.000000)
     SizeEndScale=(Base=-5.000000,Rand=15.000000)
     SpinRate=(Base=-4.000000,Rand=8.000000)
     Attraction=(X=5.000000,Y=5.000000)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke2'
}
