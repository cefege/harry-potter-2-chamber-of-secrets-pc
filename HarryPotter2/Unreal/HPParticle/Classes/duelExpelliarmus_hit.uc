//=============================================================================
// duelExpelliarmus hit
//=============================================================================
class duelExpelliarmus_hit expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=16)
    SourceHeight=(Base=16)
    AngularSpreadWidth=(Base=90)
    AngularSpreadHeight=(Base=90)
    bSteadyState=True
    Speed=(Base=10)
    Lifetime=(Base=0.5,Rand=0.25)
    ColorStart=(Base=(G=237,B=15))
    ColorEnd=(Base=(G=191,B=60))
    SizeWidth=(Base=10)
    SizeLength=(Base=10)
    ParticlesMax=100
    Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
    Rotation=(Pitch=16320)
}
