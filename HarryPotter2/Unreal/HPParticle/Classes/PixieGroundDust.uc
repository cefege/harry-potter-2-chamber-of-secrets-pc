//=============================================================================
// Pixie particles that sit on the ground floating up
//=============================================================================
class PixieGroundDust expands PixieParticles;

defaultproperties
{
     SourceWidth=(Base=15)
     SourceHeight=(Base=15)
     AngularSpreadWidth=(Base=60,Rand=20)
     AngularSpreadHeight=(Base=60,Rand=20)
     bSteadyState=True
     Speed=(Base=10,Rand=10)
     Lifetime=(Base=6.5,Rand=1.5)
     ColorStart=(Base=(R=253,G=152,B=0))
     ColorEnd=(Base=(G=202,B=40))
     SizeWidth=(Base=4,Rand=6)
     SizeLength=(Base=4,Rand=6)
     SizeEndScale=(Base=0,Rand=2)
     SpinRate=(Base=-2,Rand=4)
     DripTime=(Base=0.5,Rand=0.25)
     Attraction=(X=4,Y=4)
     Damping=0.5
     ParticlesMax=50
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Rotation=(Pitch=16323)
}
