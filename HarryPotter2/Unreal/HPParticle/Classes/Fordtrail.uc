//=============================================================================
// Particle trail for the Ford Anglia.
//=============================================================================
class Fordtrail expands FlyingFord;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=15,Rand=5)
     SourceWidth=(Base=100)
     SourceHeight=(Base=60)
     SourceDepth=(Base=60)
     Speed=(Base=10,Rand=15)
     Lifetime=(Base=1,Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Rand=4)
     SizeLength=(Rand=4)
     SizeEndScale=(Base=0,Rand=1)
     Chaos=0.75
     ChaosDelay=1
     Damping=.75
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}
