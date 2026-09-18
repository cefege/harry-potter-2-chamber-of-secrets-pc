//=============================================================================
// Potion_correct.
//=============================================================================
class Potion_correct expands ParticleFX;

defaultproperties
{
     SourceWidth=(Base=20.000000,Rand=10.000000)
     SourceHeight=(Base=20.000000,Rand=10.000000)
     SourceDepth=(Base=5.000000)
     AngularSpreadWidth=(Base=60.000000)
     AngularSpreadHeight=(Base=60.000000)
     bSteadyState=True
     speed=(Base=10.000000)
     Lifetime=(Base=2.000000,Rand=3.000000)
     ColorStart=(Base=(R=130,G=130,B=130))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=15.000000)
     SizeLength=(Base=15.000000)
     SizeEndScale=(Base=-1.000000,Rand=20.000000)
     SpinRate=(Base=-3.000000,Rand=6.000000)
     Chaos=5.000000
     Damping=0.200000
     GravityModifier=-0.020000
     ParticlesMax=75
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke2'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}
