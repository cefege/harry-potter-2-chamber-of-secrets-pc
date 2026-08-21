//=============================================================================
// GhostTrail.
//=============================================================================
class GhostTrail expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=60)
     SourceWidth=(Base=0,Rand=30)
     SourceHeight=(Base=0,Rand=100)
     SourceDepth=(Rand=30)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1,Rand=1)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=0))
     AlphaStart=(Base=0.5)
     SizeWidth=(Base=3,Rand=10)
     SizeLength=(Base=3,Rand=10)
     SizeEndScale=(Base=0,Rand=5)
     SpinRate=(Base=0.5)
     Chaos=1
     ChaosDelay=0.5
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}
