//=============================================================================
// GoldSparkle01.
//=============================================================================
class GoldSparkle01 expands TemplateSparkle01;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

//    Textures(0)=Texture'HPParticle.hp_fx.Particles.GoldSparkle01'

/*    ColorStart=(Base=(R=255,G=255,B=0),Max=(R=0,G=0,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=255,G=0,B=0))
    Textures(0)=Texture'HPParticle.hp_fx.Particles.GoldSparkle01'
*/
defaultproperties
{
    ColorStart=(Base=(R=128,G=128,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=128,G=128,B=0))
    Textures(0)=Texture'HPParticle.hp_fx.Particles.GoldSparkle01'
}
