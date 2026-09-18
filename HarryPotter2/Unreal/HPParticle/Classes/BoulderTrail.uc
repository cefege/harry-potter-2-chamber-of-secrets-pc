//=============================================================================
// BoulderTrail.
//=============================================================================
class BoulderTrail expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
	 Distribution=DIST_OwnerMesh
     ParticlesPerSec=(Base=20,Rand=20)
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
     SourceDepth=(Base=1)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=128,B=128),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=6,Rand=8)
     SizeLength=(Base=6,Rand=8)
     SizeEndScale=(Base=3)
     SpinRate=(Base=-6,Rand=6)
     Chaos=1
     GravityModifier=0.03
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke5'
}
