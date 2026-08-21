//=============================================================================
// fx for Aragog's main web breakup
//=============================================================================
class WebFxAragog expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=1000)
     SourceWidth=(Base=900,Rand=100)
     SourceHeight=(Base=50,Rand=10)
     SourceDepth=(Base=900,Rand=100)
     AngularSpreadWidth=(Base=90)
     AngularSpreadHeight=(Base=90)
     bSteadyState=True
     Speed=(Rand=20)
     Lifetime=(Base=3,Rand=2)
     ColorStart=(Base=(R=159,G=207,B=255))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=40,Rand=10)
     SizeLength=(Base=4,Rand=1)
     SpinRate=(Base=-6,Rand=6)
     Chaos=6
     GravityModifier=1
     ParticlesMax=300
     Textures(0)=Texture'HPParticle.hp_fx.Particles.webticle'
}
