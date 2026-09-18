//=============================================================================
// FSSeedSmoke.
//=============================================================================
class FSSeedSmoke expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=10,Rand=40)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     Speed=(Base=1,Rand=1)
     Lifetime=(Rand=2)
     ColorStart=(Base=(R=128,B=128),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=1,Rand=3.5)
     SizeLength=(Base=1,Rand=3.5)
     SizeEndScale=(Base=0)
     SpinRate=(Base=-6,Rand=6)
     SizeDelay=1
     SizeGrowPeriod=0.1
     Distribution=DIST_OwnerMesh
     GravityModifier=-0.01
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Dark_Smoke5'
     Style=STY_Modulated
}
