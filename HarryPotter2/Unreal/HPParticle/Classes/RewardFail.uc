class RewardFail expands Reward01;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=100)
     AngularSpreadWidth=(Rand=180)
     AngularSpreadHeight=(Rand=180)
     Speed=(Base=100)
     Lifetime=(Base=1,Rand=1)
     ColorStart=(Base=(R=134,G=134,B=134),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0,G=0,B=0))
     SizeWidth=(Base=1,Rand=20)
     SizeLength=(Base=1,Rand=20)
     Chaos=1
     Attraction=(X=-5,Y=-5,Z=-5)
     Damping=6
     GravityModifier=0.3
     ParticlesMax=80
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_5_BW'
}
