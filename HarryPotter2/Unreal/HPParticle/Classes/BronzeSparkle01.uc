//=============================================================================
// BronzeSparkle01.
//=============================================================================
class BronzeSparkle01 expands TemplateSparkle01;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

//    Textures(0)=Texture'HPParticle.hp_fx.Particles.rep_p'
//    Textures(0)=Texture'HPParticle.hp_fx.Particles.BronzeSparkle01'

/*    ColorStart=(Base=(R=255,G=128,B=0),Max=(R=0,G=0,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=255,G=0,B=0))
    Textures(0)=Texture'HPParticle.hp_fx.Particles.BronzeSparkle01'
*/
defaultproperties
{
    ColorStart=(Base=(R=128,G=64,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=128,G=64,B=0))
    Textures(0)=Texture'HPParticle.hp_fx.Particles.BronzeSparkle01'
}

