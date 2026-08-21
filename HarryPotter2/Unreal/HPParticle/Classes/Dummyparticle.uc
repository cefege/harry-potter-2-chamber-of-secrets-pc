//=============================================================================
// Dummyparticle.
//=============================================================================
class Dummyparticle expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Rand=10.000000)
     SourceWidth=(Base=50.000000)
     SourceHeight=(Base=50.000000)
     speed=(Base=15.000000,Rand=30.000000)
     Lifetime=(Base=5.000000)
     ColorStart=(Base=(R=149,G=166,B=244),Rand=(R=28,G=19,B=196))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=6.000000,Rand=8.000000)
     SizeLength=(Base=6.000000,Rand=8.000000)
     SizeEndScale=(Base=3.000000)
     SpinRate=(Base=0.500000)
     SizeDelay=2.000000
     Chaos=1.000000
     ChaosDelay=0.500000
     Textures(0)=FireTexture'HPparticle.HP_FX.Particles.spin'
}

