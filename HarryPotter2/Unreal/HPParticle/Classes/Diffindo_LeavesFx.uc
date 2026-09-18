//=============================================================================
// leaves for diffindo hits
//=============================================================================
class Diffindo_LeavesFx expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=1000)
     SourceWidth=(Base=128)
     SourceHeight=(Base=0)
     SourceDepth=(Base=128)
     AngularSpreadWidth=(Base=90,Rand=90)
     AngularSpreadHeight=(Base=90,Rand=90)
     bSteadyState=True
     Lifetime=(Base=4,Rand=2)
     ColorStart=(Base=(R=216,G=255,B=168),Rand=(R=19,G=249))
     ColorEnd=(Base=(R=209,G=253,B=2),Rand=(R=196,G=255,B=15))
     AlphaEnd=(Base=1)
     SizeWidth=(Base=12,Rand=6)
     SizeLength=(Base=12,Rand=6)
     SpinRate=(Base=-1,Rand=2)
     Chaos=3
     Damping=1
     GravityModifier=0.01
     ParticlesMax=32
     Textures(0)=Texture'HPParticle.hp_fx.Particles.leaf'
     Rotation=(Pitch=49472)
     Style=STY_Masked
}
