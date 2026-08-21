//=============================================================================
// FX that rise from Griffindor sword blade
//=============================================================================
class SwordBladeFX expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=25)
     SourceWidth=(Base=6)
     SourceHeight=(Base=6)
     bSteadyState=True
     Speed=(Base=15,Rand=10)
     Lifetime=(Base=2,Rand=1)
     ColorStart=(Base=(R=254,G=27,B=1),Rand=(R=254,G=186,B=5))
     ColorEnd=(Base=(R=247,G=255,B=77),Rand=(R=254,G=159,B=18))
     SizeWidth=(Base=2,Rand=6)
     SizeLength=(Base=2,Rand=6)
     bSystemRelative=True
     Attraction=(X=20,Y=20)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}
