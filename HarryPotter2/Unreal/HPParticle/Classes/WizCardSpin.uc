//=============================================================================
// WizCardSpin.
//=============================================================================
class WizCardSpin expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
	 Distribution=DIST_OwnerMesh
     ParticlesPerSec=(Base=80,Rand=80)
     SourceWidth=(Base=20,Rand=50)
     SourceHeight=(Base=20,Rand=50)
     SourceDepth=(Base=20,Rand=50)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=0,Rand=100)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=0,G=0,B=255),Rand=(R=255,G=255,B=255))
     ColorEnd=(Base=(R=0,B=255))
     SizeWidth=(Base=2,Rand=4)
     SizeLength=(Base=2,Rand=4)
     SizeEndScale=(Base=0,Rand=6)
     Chaos=1
     Attraction=(X=5,Y=5)
     Damping=2
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_01'
}
