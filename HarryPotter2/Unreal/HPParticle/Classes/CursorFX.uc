//=============================================================================
// Cursor particle fx
//=============================================================================
class CursorFX expands ParticleFX;

defaultproperties
{
    SourceWidth=(Base=0)
    SourceHeight=(Base=0)
    AngularSpreadWidth=(Base=0)
    AngularSpreadHeight=(Base=0)
    bSteadyState=True
    Speed=(Base=40)
    Lifetime=(Base=2)
    ColorStart=(Base=(R=121,G=255,B=11))
    ColorEnd=(Base=(R=0))
    SizeWidth=(Base=16)
    SizeLength=(Base=16)
    SizeEndScale=(Base=0)
    SpinRate=(Base=1,Rand=8)
    SizeDelay=1
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_03'
    Physics=PHYS_Rotating
    bFixedRotationDir=True
    RotationRate=(Pitch=100000)
}
