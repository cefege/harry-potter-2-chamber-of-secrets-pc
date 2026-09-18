//=============================================================================
// Ashes resulting from Fawkes Explosion
//=============================================================================
class FawkesToAshes expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=400)
     SourceWidth=(Base=30,Rand=15)
     SourceHeight=(Rand=10)
     SourceDepth=(Base=10,Rand=10)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     Speed=(Base=100,Rand=100)
     Lifetime=(Base=2,Rand=0.5)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0,G=64,B=128))
     AlphaEnd=(Base=1)
     SizeWidth=(Base=4,Rand=4)
     SizeLength=(Base=4,Rand=4)
     SizeEndScale=(Base=0.1)
     SpinRate=(Base=-6,Rand=12)
     DripTime=(Base=0.1)
     Chaos=20
     Elasticity=0.1
     Attraction=(X=3,Y=3)
     Damping=5
     GravityModifier=0.7
     ParticlesMax=200
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke5'
     bDynamicLight=True
     Rotation=(Pitch=16368)
     bSelected=True
}
