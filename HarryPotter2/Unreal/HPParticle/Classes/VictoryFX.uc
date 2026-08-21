//=============================================================================
// VictoryFX.
//=============================================================================
class VictoryFX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=1000)
    SourceWidth=(Base=20)
    SourceHeight=(Base=20)
    SourceDepth=(Base=20)
    AngularSpreadWidth=(Base=180)
    AngularSpreadHeight=(Base=180)
    bSteadyState=True
    Speed=(Base=200,Rand=20)
    Lifetime=(Rand=1)
    ColorStart=(Base=(R=254,G=176,B=16))
    ColorEnd=(Base=(R=254,G=87,B=7))
    SizeEndScale=(Base=-1,Rand=2)
    SpinRate=(Base=-4,Rand=4)
    Chaos=10
    Damping=8
    ParticlesMax=400
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_BW'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}
