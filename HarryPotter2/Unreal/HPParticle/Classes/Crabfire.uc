//=============================================================================
// Crabfire.
//=============================================================================
class Crabfire expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=70,Rand=30)
     SourceWidth=(Base=0,Rand=15)
     SourceHeight=(Base=0,Rand=15)
     SourceDepth=(Rand=15)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=20,Rand=10)
     Lifetime=(Rand=1)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(G=200,B=125))
     SizeWidth=(Rand=4)
     SizeLength=(Rand=4)
     SizeEndScale=(Base=0.1,Rand=0.0001)
     SpinRate=(Base=-6,Rand=12)
     //bSystemRelative=True
     Chaos=2
     Elasticity=0.01
     Damping=5
     GravityModifier=0.5
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
     bDynamicLight=True
     Rotation=(Pitch=16368)
     bSelected=True
}
