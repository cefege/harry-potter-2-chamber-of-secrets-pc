//=============================================================================
// badkeys.
//=============================================================================
class badkeys expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=0,Rand=5)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=75,Rand=125)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=128,G=0,B=128),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0,G=255))
     AlphaEnd=(Base=1)
     SizeWidth=(Base=50)
     SizeLength=(Base=50)
     SizeEndScale=(Base=0,Rand=2)
     SpinRate=(Base=-1,Rand=1)
     Chaos=1
     Attraction=(X=3,Y=3,Z=3)
     Damping=1
     Textures(0)=Texture'HPParticle.hp_fx.General.AngryKeys_A00'
}
