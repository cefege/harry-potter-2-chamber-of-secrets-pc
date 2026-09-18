//=============================================================================
// ChessTrail.
//=============================================================================
class ChessTrail expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=30,Rand=30)
     SourceWidth=(Base=0,Rand=60)
     SourceHeight=(Base=0,Rand=10)
     SourceDepth=(Rand=60)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1)
     Lifetime=(Rand=2)
     ColorStart=(Base=(G=255,B=255),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Rand=16)
     SizeLength=(Rand=16)
     SizeEndScale=(Base=0,Rand=3)
     Chaos=1
     ChaosDelay=0.5
     Distribution=DIST_OwnerMesh
     GravityModifier=-0.05
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}
