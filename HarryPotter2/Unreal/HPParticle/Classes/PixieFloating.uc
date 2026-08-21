//=============================================================================
// Pixie particles that fly/float off from cornish pixies
//=============================================================================
class PixieFloating expands PixieParticles;

defaultproperties
{
    ParticlesPerSec=(Base=10,Rand=5)
    SourceWidth=(Base=20,Rand=5)
    SourceHeight=(Base=20,Rand=5)
    SourceDepth=(Base=20,Rand=5)
    AngularSpreadWidth=(Base=30)
    AngularSpreadHeight=(Base=30)
    bSteadyState=True
    Speed=(Base=10,Rand=10)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(R=253,G=152,B=0))
    ColorEnd=(Base=(G=202,B=40))
    SizeWidth=(Base=4,Rand=6)
    SizeLength=(Base=4,Rand=6)
    SizeEndScale=(Base=0,Rand=2)
    SpinRate=(Base=-2,Rand=4)
//    Attraction=(X=4,Y=4)
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
    Rotation=(Pitch=16323)
}
