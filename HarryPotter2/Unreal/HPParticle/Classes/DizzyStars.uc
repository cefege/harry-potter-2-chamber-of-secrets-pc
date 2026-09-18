//=============================================================================
// DizzyStars.
//=============================================================================
class DizzyStars expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=1,Rand=10)
     SourceWidth=(Base=30,Rand=30)
     SourceHeight=(Base=5,Rand=5)
     SourceDepth=(Base=30,Rand=30)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     bSteadyState=True
     Speed=(Base=0,Rand=20)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=244,G=253,B=85),Rand=(R=138,G=138,B=138))
     ColorEnd=(Base=(G=26,B=31),Rand=(R=127,G=127,B=127))
     SizeWidth=(Base=2,Rand=4)
     SizeLength=(Base=2,Rand=4)
     SpinRate=(Base=-1,Rand=2)
     AlphaDelay=0.7
     Chaos=1
     Attraction=(X=10,Y=10)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Goldstar01'
     Rotation=(Pitch=16640)
}
