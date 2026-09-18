//=============================================================================
// Venom that comes out of Basilisks mouth.
//=============================================================================
class SnakeVenomFX expands particlefx;

defaultproperties
{
     ParticlesPerSec=(Base=45)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=0)
     ColorStart=(Base=(R=134,G=96,B=223),Rand=(R=88,G=70,B=191))
     ColorEnd=(Base=(R=214,G=32,B=223),Rand=(R=230,G=72,B=155))
     SizeWidth=(Base=16,Rand=8)
     SizeLength=(Base=16,Rand=8)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_1'

     Lifetime=(Base=0.175)
}
