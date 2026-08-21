//=============================================================================
// GoldstarFinal. fx for final challenge star
//=============================================================================
class GoldstarFinal expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=20,Rand=5)
     SourceWidth=(Rand=10)
     SourceHeight=(Rand=10)
     SourceDepth=(Base=5,Rand=10)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=80,Rand=30)
     Lifetime=(Base=2,Rand=1)
     ColorStart=(Base=(R=244,G=253,B=85),Rand=(R=138,G=138,B=138))
     ColorEnd=(Base=(G=26,B=31),Rand=(R=127,G=127,B=127))
     SizeWidth=(Base=4,Rand=6)
     SizeLength=(Base=4,Rand=6)
     SpinRate=(Base=-1,Rand=2)
     AlphaDelay=0.7
     Attraction=(X=3,Y=3,Z=3)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Goldstar01'
     Rotation=(Pitch=16640)
}
