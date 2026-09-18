//=============================================================================
// LumosPlatform.
//=============================================================================
class LumosPlatform expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=25,Rand=5)
     SourceWidth=(Base=96)
     SourceHeight=(Base=16)
     SourceDepth=(Base=96)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     speed=(Base=0)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=250,G=206,B=1))
     ColorEnd=(Base=(R=185,G=151,B=255))
     SizeWidth=(Base=1.25)
     SizeLength=(Base=1.25)
     SizeEndScale=(Base=60,Rand=10)
     SpinRate=(Base=-2,Rand=4)
     Chaos=1
     Textures(0)=Texture'HPParticle.hp_fx.General.CandleF'
     CollisionHeight=20
}
