//=============================================================================
// Cauldron_Mixed.  Reaction fx when potion is mixed in a teacher cauldron
//=============================================================================
class Cauldron_Mixed expands CauldronFX;

defaultproperties
{
    ParticlesPerSec=(Base=75,Rand=35)
    SourceWidth=(Base=30)
    SourceHeight=(Base=30)
    AngularSpreadWidth=(Base=0)
    AngularSpreadHeight=(Base=0)
    bSteadyState=True
    Speed=(Base=35,Rand=25)
    Lifetime=(Base=3,Rand=2)
    ColorStart=(Base=(G=255,B=255))
    ColorEnd=(Base=(G=255,B=255))
    SizeWidth=(Base=6,Rand=6)
    SizeLength=(Base=6,Rand=6)
    SizeEndScale=(Base=0,Rand=5)
    SpinRate=(Base=-4,Rand=8)
    Chaos=5
    Attraction=(X=5,Y=5)
    Damping=0.75
    ParticlesMax=100
    Textures(0)=FireTexture'HPParticle.hp_fx.Particles.SpinG'
    Rotation=(Pitch=16323)
    
}
