//=============================================================================
// Rictusempra_fly.
//=============================================================================
class Rictusempra_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=30)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=40)
     Lifetime=(Base=2)
     ColorStart=(Base=(R=207,G=46,B=50))
     ColorEnd=(Base=(G=111,B=55))
     SizeWidth=(Base=16)
     SizeLength=(Base=16)
     SizeEndScale=(Base=0)
     SpinRate=(Base=1,Rand=8)
     SizeDelay=1
     Damping=2
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Physics=PHYS_Rotating
     bFixedRotationDir=True
     RotationRate=(Yaw=200000,Roll=200000)
}
