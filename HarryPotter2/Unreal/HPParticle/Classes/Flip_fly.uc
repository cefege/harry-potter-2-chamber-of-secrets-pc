//=============================================================================
// Flip_fly.
//=============================================================================
class Flip_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=100)
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=75)
     ColorStart=(Base=(R=254,G=142,B=61))
     ColorEnd=(Base=(R=201,G=85,B=46))
     SizeWidth=(Base=4,Rand=9)
     SizeLength=(Base=4,Rand=9)
     SizeEndScale=(Base=7)
     SpinRate=(Base=-3,Rand=6)
     AlphaGrowPeriod=0.1
     Attraction=(X=10,Y=10,Z=10)
     Damping=1
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     LastUpdateLocation=(X=132,Y=-348,Z=-44.50056)
     LastEmitLocation=(X=132,Y=-348,Z=-44.50056)
     LastUpdateRotation=(Pitch=16464)
     EmissionResidue=0.03827667
     Age=1139.42
     ParticlesEmitted=53159
     bDynamicLight=True
     Tag=Dummyparticle
     Location=(X=132,Y=-348,Z=-44.50056)
     OldLocation=(Z=32)
     bSelected=True
}
