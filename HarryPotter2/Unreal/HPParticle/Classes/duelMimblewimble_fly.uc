//=============================================================================
// duelMimblewimble_fly.
//=============================================================================
class duelMimblewimble_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=30)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=10)
     AngularSpreadHeight=(Base=10)
	 bSteadyState=True
     Speed=(Base=30,Rand=15)
     Lifetime=(Base=1)
     ColorStart=(Base=(R=34,G=67,B=255))
     ColorEnd=(Base=(R=113,G=6,B=164))
     SizeWidth=(Base=10)
     SizeLength=(Base=10)
     SizeEndScale=(Base=-1)
     SpinRate=(Base=5,Rand=10)
	 SizeDelay=1
     Damping=2
     Chaos=3
     GravityModifier=0.05
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}
