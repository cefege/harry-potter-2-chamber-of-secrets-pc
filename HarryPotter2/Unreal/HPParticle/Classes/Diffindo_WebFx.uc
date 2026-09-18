//=============================================================================
// FX for Aragog Web Anchor
//=============================================================================
class Diffindo_WebFx expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=17,Rand=4)
     SourceWidth=(Base=40)
     SourceHeight=(Base=40)
     SourceDepth=(Base=200)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=-15,Rand=15)
     Lifetime=(Base=1,Rand=0.5)
     ColorStart=(Base=(G=255,B=255),Rand=(R=255))
     ColorEnd=(Base=(R=128,B=128),Rand=(R=255,G=145,B=53))
     SizeWidth=(Base=25,Rand=5)
     SizeLength=(Base=25,Rand=5)
     SizeEndScale=(Base=0,Rand=1)
     SpinRate=(Base=-2,Rand=4)
     DripTime=(Base=0.5)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=48995)
}
