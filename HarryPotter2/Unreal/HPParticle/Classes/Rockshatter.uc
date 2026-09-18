//=============================================================================
// rockshatter
//=============================================================================
class Rockshatter expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=5000)
     AngularSpreadWidth=(Base=60)
     AngularSpreadHeight=(Base=60)
     bSteadyState=True
     Speed=(Base=150,Rand=50)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=199,G=167,B=120),Rand=(R=173,G=146,B=120))
     ColorEnd=(Base=(R=102,G=100,B=77),Rand=(R=156,G=109,B=82))
     SizeWidth=(Base=6,Rand=4)
     SizeLength=(Base=6,Rand=4)
     SizeEndScale=(Base=0)
     SpinRate=(Base=-3,Rand=6)
     Damping=3
     GravityModifier=0.75
     ParticlesMax=20
     Textures(0)=Texture'HPParticle.hp_fx.Particles.rockpiece'
     Style=STY_Masked
     AmbientGlow=200
}
