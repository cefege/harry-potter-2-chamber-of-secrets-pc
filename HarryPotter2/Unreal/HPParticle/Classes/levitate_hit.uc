//=============================================================================
// levitate_hit.
//=============================================================================
class levitate_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=5.000000)
     speed=(Base=20.000000,Rand=30.000000)
     Lifetime=(Base=0.500000)
     ColorStart=(Base=(R=149,G=166,B=244),Rand=(R=28,G=19,B=196))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=120.000000,Rand=50.000000)
     SizeLength=(Base=120.000000,Rand=50.000000)
     SpinRate=(Base=2.000000)
     SizeDelay=2.000000
     Chaos=3.000000
     ChaosDelay=0.500000
     ParticlesAlive=5
     ParticlesMax=5
     Textures(0)=FireTexture'HPParticle.hp_fx.Particles.spin'
     Rotation=(Pitch=16640)
 	 bRotateToDesired=true;

}
