//=============================================================================
// fx for Aragog's main web, dust/smoke 
//=============================================================================
class WebDustAragog expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=1000)
     SourceWidth=(Base=900,Rand=100)
     SourceHeight=(Base=100,Rand=50)
     SourceDepth=(Base=900,Rand=100)
     AngularSpreadWidth=(Base=90)
     AngularSpreadHeight=(Base=90)
     Speed=(Base=20)
     Lifetime=(Base=4,Rand=2)
     ColorStart=(Base=(R=126,G=126,B=131))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=30,Rand=20)
     SizeLength=(Base=30,Rand=20)
     SizeEndScale=(Base=0)
     SpinRate=(Base=-1,Rand=2)
     Chaos=4
     GravityModifier=0.2
     ParticlesMax=200
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke1'
}
