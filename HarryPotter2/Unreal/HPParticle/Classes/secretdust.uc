//=============================================================================
// particle for chambersecrets.
//=============================================================================
class secretdust expands ParticleFX;

defaultproperties
{   
    ParticlesPerSec=(Base=28)
    SourceWidth=(Base=150,Rand=10)
    SourceDepth=(Base=28)
    bSteadyState=True
    Speed=(Base=-0.5,Rand=1)
    Lifetime=(Base=8,Rand=4)
    ColorStart=(Base=(R=197,G=198,B=193))
    ColorEnd=(Base=(G=255,B=210))
    SizeWidth=(Base=0.5,Rand=1)
    SizeLength=(Base=0.5,Rand=1)
    Chaos=0.2
    Gravity=(Y=0.5)
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_Neutral'
    Rotation=(Pitch=16208)
}
