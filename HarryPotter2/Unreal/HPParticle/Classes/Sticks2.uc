//=============================================================================
// Stick fx for roots explode and or bowtruckles
//=============================================================================
class Sticks2 expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceDepth=(Base=10)
     AngularSpreadWidth=(Base=90,Rand=90)
     AngularSpreadHeight=(Base=90,Rand=90)
     Speed=(Base=200,Rand=40)
     Lifetime=(Base=1.5,Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(G=255,B=255))
     AlphaEnd=(Base=1)
     SizeWidth=(Base=12,Rand=6)
     SizeLength=(Base=12,Rand=6)
     SpinRate=(Base=-2,Rand=4)
     Elasticity=0.1
     Damping=3.2
     GravityModifier=0.5
     ParticlesMax=8
     Textures(0)=Texture'HPParticle.hp_fx.Particles.twig2'
     Style=STY_Masked
}
