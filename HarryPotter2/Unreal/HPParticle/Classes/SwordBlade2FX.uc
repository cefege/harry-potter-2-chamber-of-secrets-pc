//=============================================================================
// Skurge_fly.
//=============================================================================
class SwordBlade2FX expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=30,Rand=10)
     SourceWidth=(Base=6)
     SourceHeight=(Base=6)
     AngularSpreadWidth=(Base=10)
     AngularSpreadHeight=(Base=10)
     Speed=(Base=30,Rand=15)
     Lifetime=(Base=0.3)
     //ColorStart=(Base=(R=34,G=67,B=255))
     //ColorEnd=(Base=(R=113,G=6,B=164))
     ColorStart=(Base=(R=254,G=27,B=1),Rand=(R=254,G=186,B=5))
     ColorEnd=(Base=(R=247,G=255,B=77),Rand=(R=254,G=159,B=18))
     SizeWidth=(Base=25)
     SizeLength=(Base=25)
     SizeEndScale=(Base=-1)
     SpinRate=(Base=5,Rand=10)
     Chaos=3
     GravityModifier=0.05
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}
