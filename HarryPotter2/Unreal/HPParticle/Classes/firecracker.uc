//=============================================================================
// Firecracker.
//=============================================================================
class firecracker expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=90,Rand=90)
     AngularSpreadHeight=(Base=90,Rand=90)
     bSteadyState=True
     speed=(Base=300,Rand=100)
     Lifetime=(Base=1.25,Rand=0.5)
     ColorStart=(Base=(R=29,G=17,B=255))
     ColorEnd=(Base=(G=128))
     SizeWidth=(Base=3,Rand=10)
     SizeLength=(Base=3,Rand=10)
     SizeEndScale=(Base=-1,Rand=2)
     SpinRate=(Base=-6,Rand=12)
     AlphaDelay=1
     ColorDelay=0.4
     Chaos=10
     Damping=6
     GravityModifier=0.001
     Gravity=(Z=-60)
     ParticlesMax=25
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_BW'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}
