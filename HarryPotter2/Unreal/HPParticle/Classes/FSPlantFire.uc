//=============================================================================
// FSPlantFire.
//=============================================================================
class FSPlantFire expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=80,Rand=80)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=0,Rand=5)
     Lifetime=(Rand=1)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     SizeWidth=(Base=1,Rand=5)
     SizeLength=(Base=1,Rand=5)
     SizeEndScale=(Base=0,Rand=16)
     SpinRate=(Base=-6,Rand=6)
     Chaos=1
     Distribution=DIST_OwnerMesh
     GravityModifier=-0.05
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
}
