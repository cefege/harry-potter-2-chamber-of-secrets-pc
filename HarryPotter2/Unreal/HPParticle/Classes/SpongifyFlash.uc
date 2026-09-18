//=============================================================================
// Flash when spongify pad is activated.
//=============================================================================
class SpongifyFlash expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=96)
    SourceHeight=(Base=96)
    AngularSpreadWidth=(Base=90)
    AngularSpreadHeight=(Base=90)
    bSteadyState=True
    Speed=(Base=10)
    Lifetime=(Base=0.5,Rand=0.25)
    ColorStart=(Base=(R=120,G=34,B=206))
    ColorEnd=(Base=(R=133,G=18,B=194))
    SizeWidth=(Base=16,Rand=16)
    SizeLength=(Base=16,Rand=16)
    ParticlesMax=100
    Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
    Rotation=(Pitch=16320)
}
