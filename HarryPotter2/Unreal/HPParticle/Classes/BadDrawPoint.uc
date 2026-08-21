//=============================================================================
// BadDrawPoint.
//=============================================================================
class BadDrawPoint expands GoodDrawPoint;

/*     Distribution=DIST_Uniform
     ParticlesPerSec=(Base=2.00000)
     SourceWidth=(Base=0.000000)
     SourceHeight=(Base=0.000000)
     AngularSpreadWidth=(Base=0.000000)
     AngularSpreadHeight=(Base=0.000000)
     speed=(Base=0.000000)
     Lifetime=(Base=60.000000)
     ColorStart=(Base=(R=255,G=255,B=255))
     ColorEnd=(Base=(R=255,G=255,B=255))
     SizeEndScale=(Base=1)
	 SizeWidth=(Base=5.0)
	 SizeLength=(Base=5.0)
*/

/*	Distribution=DIST_uniform
    ParticlesPerSec=(Base=3.000000,Rand=0.000000)
    SourceWidth=(Base=0.100000)
    SourceHeight=(Base=0.100000)
    AngularSpreadWidth=(Base=180.000000)
    AngularSpreadHeight=(Base=180.000000)
    bSteadyState=True
    speed=(Base=0.000000,Rand=0.000000)
    Lifetime=(Base=10.0000)
    ColorStart=(Base=(R=255,G=255,B=255),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=255,G=255,B=255))
	AlphaStart=1.0
	AlphaEnd=1.0
    Textures(0)=Texture'HPParticle.hp_fx.Particles.SilverSparkle01'
    SizeWidth=(Base=12.000000,Rand=0.000000)
    SizeLength=(Base=12.000000,Rand=0.000000)
    SizeEndScale=(Base=1.000000)
    SpinRate=(Base=-1.000000,Rand=2.000000)
    SizeDelay=0.000000
    ParticlesAlive=200
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
    Name=Dummyparticle0*/

//     Chaos=5.000000
//     Damping=0.200000
//     GravityModifier=-0.020000

defaultproperties
{
     Distribution=DIST_Uniform
     ParticlesPerSec=(Base=2.00000)
     SourceWidth=(Base=0.000000)
     SourceHeight=(Base=0.000000)
     AngularSpreadWidth=(Base=60.000000)
     AngularSpreadHeight=(Base=60.000000)
     bSteadyState=True
     speed=(Base=10.000000)
     Lifetime=(Base=3.000000,Rand=3.000000)
     ColorStart=(Base=(R=255,G=0,B=0))
     ColorEnd=(Base=(R=255,G=0,B=0))
     SizeEndScale=(Base=1.000000,Rand=5.000000)
	 SizeWidth=(Base=5.0)
	 SizeLength=(Base=5.0)
     SpinRate=(Base=-3.000000,Rand=6.000000)
     GravityModifier=0.10000
}

