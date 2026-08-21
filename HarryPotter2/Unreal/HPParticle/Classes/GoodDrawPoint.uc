//=============================================================================
// GoodDrawPoint.
//=============================================================================
class GoodDrawPoint expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

//    Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_5_BW'
//    Textures(0)=Texture'HPParticle.hp_fx.Particles.TemplateSparkle01'
//    Textures(0)=Texture'HPParticle.hp_fx.Particles.f_spark'

/*    ColorStart=(Base=(R=0,G=255,B=0),Max=(R=0,G=0,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=0,G=0,B=0))
*/

/*
	Distribution=DIST_uniform
    ParticlesPerSec=(Base=3.000000,Rand=0.000000)
    SourceWidth=(Base=0.100000)
    SourceHeight=(Base=0.100000)
    AngularSpreadWidth=(Base=180.000000)
    AngularSpreadHeight=(Base=180.000000)
    bSteadyState=True
    speed=(Base=0.000000,Rand=0.000000)
    Lifetime=(Base=0.150000)
    ColorStart=(Base=(R=0,G=255,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=0,G=0,B=0))
    SizeWidth=(Base=12.000000,Max=0.000000,Rand=0.000000)
    SizeLength=(Base=12.000000,Max=0.000000,Rand=0.000000)
    SizeEndScale=(Base=1.000000)
    SpinRate=(Base=-1.000000,Rand=2.000000)
    SizeDelay=0.000000
    ParticlesAlive=200
    Textures(0)=Texture'HPParticle.hp_fx.Particles.TemplateSparkle01'
    LastUpdateLocation=(Z=32.000000)
    LastEmitLocation=(Z=32.000000)
    LastUpdateRotation=(Pitch=16360)
    EmissionResidue=0.386094
    Age=368.458679
    bDynamicLight=True
    Level=LevelInfo'MyLevel.LevelInfo0'
    Tag=Dummyparticle
    Region=(Zone=LevelInfo'MyLevel.LevelInfo0',iLeaf=2,ZoneNumber=1)
    Location=(Z=32.000000)
    Rotation=(Pitch=16640)
    OldLocation=(Z=32.000000)
    Name=Dummyparticle0
*/

/*    ColorStart=(Base=(R=128,G=128,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=128,G=128,B=0))
*/

defaultproperties
{
     Distribution=DIST_Uniform
     ParticlesPerSec=(Base=2.00000)
     SourceWidth=(Base=0.000000)
     SourceHeight=(Base=0.000000)
     AngularSpreadWidth=(Base=0.000000)
     AngularSpreadHeight=(Base=0.000000)
     speed=(Base=0.000000)
     Lifetime=(Base=10.000000)
     SizeEndScale=(Base=1)
	 SizeWidth=(Base=5.0)
	 SizeLength=(Base=5.0)

    ColorStart=(Base=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=0,G=0,B=0))
}

