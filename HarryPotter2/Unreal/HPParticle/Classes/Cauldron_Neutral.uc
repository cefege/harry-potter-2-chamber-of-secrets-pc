//=============================================================================
// Cauldron_Neutral.  light smoke fx coming out of teachers cauldron
//=============================================================================
class Cauldron_Neutral expands CauldronFX;

defaultproperties
{
    ParticlesPerSec=(Base=20,Rand=10)
    SourceWidth=(Base=30)
    SourceHeight=(Base=30)
    AngularSpreadWidth=(Base=20,Rand=15)
    AngularSpreadHeight=(Base=20,Rand=15)
    bSteadyState=True
    Speed=(Base=2,Rand=20)
    Lifetime=(Base=4,Rand=2)
    ColorStart=(Base=(R=166,G=162,B=247))
    ColorEnd=(Base=(R=40,G=47,B=159))
    SizeWidth=(Base=4,Rand=6)
    SizeLength=(Base=4,Rand=6)
    SizeEndScale=(Base=0.001,Rand=5)
    SpinRate=(Base=-2,Rand=4)
    Chaos=8
    ChaosDelay=2
    Damping=0.25
    GravityModifier=0.01
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke3'
    Rotation=(Pitch=16323)
    AmbientSound=Sound'HPSounds.Exec_demo_level_SFX.cauldron_bubbling'
    SoundRadius=18
    SoundVolume=224
    SoundPitch=90
}
