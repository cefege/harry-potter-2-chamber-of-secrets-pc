//=============================================================================
// WizCrackSparkle.
//=============================================================================
class WizCrackSparkle expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=50,Rand=50)
     SourceWidth=(Base=0,Rand=50)
     SourceHeight=(Base=0,Rand=30)
     SourceDepth=(Rand=50)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1)
     Lifetime=(Base=0,Rand=1)
     ColorStart=(Base=(G=0,B=0),Rand=(R=255,G=255))
     ColorEnd=(Base=(G=128),Rand=(R=128,G=128,B=128))
     SizeWidth=(Base=1,Rand=3)
     SizeLength=(Base=1,Rand=3)
     SizeEndScale=(Base=0,Rand=3)
     SpinRate=(Base=-6,Rand=6)
     Chaos=1
     Attraction=(X=-0.01,Y=-0.01,Z=-0.01)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_02'
     bDynamicLight=True
}
