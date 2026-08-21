//=============================================================================
// Wing_fly.
//=============================================================================
class Wing_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=0,Rand=20)
     SourceWidth=(Base=0,Rand=10)
     SourceHeight=(Base=0,Rand=10)
     SourceDepth=(Rand=10)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=30)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=6,Rand=6)
     SizeLength=(Base=6,Rand=6)
     SizeEndScale=(Base=0,Rand=4)
     SpinRate=(Base=-3,Rand=6)
     Chaos=1
     Damping=3
     GravityModifier=-0.05
     Textures(0)=Texture'HPParticle.hp_fx.Particles.White_Feather'
     LastUpdateLocation=(X=5.118073,Y=4.56073,Z=36.09917)
     LastEmitLocation=(X=5.118073,Y=4.56073,Z=36.09917)
     EmissionResidue=0.9394684
     Age=20.12129
     ParticlesEmitted=2012
     bDynamicLight=True
     Tag=Wing_fly
     Location=(X=5.118073,Y=4.56073,Z=36.09917)
     Rotation=(Pitch=0)
     OldLocation=(X=5.118073,Y=-1.439267,Z=36.09917)
     bSelected=True
}
