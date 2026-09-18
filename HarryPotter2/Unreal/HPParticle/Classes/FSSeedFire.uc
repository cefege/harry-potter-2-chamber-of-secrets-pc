//=============================================================================
// FSSeedFire.
//=============================================================================
class FSSeedFire expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=20,Rand=20)
     SourceWidth=(Base=0,Rand=1)
     SourceHeight=(Base=0,Rand=1)
     SourceDepth=(Rand=1)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     Speed=(Base=0,Rand=8)
     Lifetime=(Rand=1)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     SizeWidth=(Base=1,Rand=1)
     SizeLength=(Base=1,Rand=1)
     SizeEndScale=(Base=0,Rand=1)
     SpinRate=(Base=-6,Rand=6)
     Chaos=1
     Damping=5
     Distribution=DIST_OwnerMesh
     GravityModifier=-0.05
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
}
