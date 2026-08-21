//=============================================================================
// Jellybean_Pickup.
//=============================================================================
class Jellybean_Pickup expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=20.000000)
     AngularSpreadWidth=(Base=90.000000)
     AngularSpreadHeight=(Base=90.000000)
     speed=(Base=20.000000,Rand=15.000000)
     Lifetime=(Base=1.500000)
     ColorStart=(Base=(R=201,G=163,B=222))
     ColorEnd=(Base=(R=0))
     SizeEndScale=(Base=2.000000)
     SpinRate=(Base=1.000000,Rand=20.000000)
     SizeDelay=1.000000
     Attraction=(X=20.000000,Y=20.000000)
     ParticlesMax=50
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_4'
}
