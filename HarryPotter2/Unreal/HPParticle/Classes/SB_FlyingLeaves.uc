//=============================================================================
// SB_FlyingLeaves.
//=============================================================================
class SB_FlyingLeaves expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=5,Rand=15)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=25,Rand=25)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=4,Rand=8)
     SizeLength=(Base=4,Rand=8)
     SizeEndScale=(Base=0,Rand=3)
     SpinRate=(Base=-8,Rand=8)
     Chaos=1
     Attraction=(X=-0.05,Y=-0.05,Z=-0.05)
     Damping=1
     ParticlesMax=20
     Textures(0)=Texture'HPParticle.hp_fx.General.SPIKYBUSH_leaf'
}
