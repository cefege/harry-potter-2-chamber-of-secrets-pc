//=============================================================================
// SpellTarget_NonLock.
//=============================================================================
class SpellTarget_NonLock expands SpellTarget;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=30)
     SourceWidth=(Base=0,Rand=0)
     SourceHeight=(Base=0,Rand=0)
     SourceDepth=(Base=0,Rand=0)
     Speed=(Rand=0)
     ColorStart=(Rand=(R=0,G=0))
     ColorEnd=(Base=(R=255,G=128,B=0))
     Damping=0
}
