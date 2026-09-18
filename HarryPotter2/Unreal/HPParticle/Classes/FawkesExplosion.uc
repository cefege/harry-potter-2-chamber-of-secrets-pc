//=============================================================================
// Burst of Flames when Fawkes Explodes
//=============================================================================
class FawkesExplosion expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=400)
     SourceWidth=(Base=30,Rand=15)
     SourceHeight=(Base=5,Rand=15)
     SourceDepth=(Base=30,Rand=15)
     AngularSpreadWidth=(Base=90)
     AngularSpreadHeight=(Base=90)
     Speed=(Base=100,Rand=50)
     Lifetime=(Base=0.5,Rand=1)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(G=200,B=125))
     SizeWidth=(Base=15,Rand=5)
     SizeLength=(Base=15,Rand=5)
     SizeEndScale=(Base=0.1,Rand=0.0001)
     SpinRate=(Base=-6,Rand=12)
     SizeDelay=0.6
     Chaos=2
     Damping=5
     GravityModifier=-0.5
     ParticlesMax=500
     Textures(0)=FireTexture'HPParticle.hp_fx.Spells.GoldSparkle01'
     bDynamicLight=True
     Rotation=(Pitch=16368)
     bSelected=True
     CollisionRadius=35
     CollisionHeight=40
}
