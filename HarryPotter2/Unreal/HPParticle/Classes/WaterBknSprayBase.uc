//=============================================================================
// WaterBknSpray.
//=============================================================================
class WaterBknSprayBase expands WaterShowerFX;

defaultproperties
{
     ParticlesPerSec=(Base=5.000000,Rand=25.000000)
     AngularSpreadWidth=(Base=20.000000,Rand=20.000000)
     AngularSpreadHeight=(Base=60.000000,Rand=20.000000)
     speed=(Base=170.000000)
     Lifetime=(Base=0.500000)
     ColorStart=(Base=(R=0,G=206,B=83),Rand=(R=99,G=68,B=39))
     ColorEnd=(Base=(R=61,G=88,B=70),Rand=(R=47,G=77,B=85))
     SizeWidth=(Base=4.000000)
     AlphaDelay=0.400000
     ColorDelay=0.200000
     Attraction=(X=0.000000,Y=0.000000)
     Damping=0.000000
     GravityModifier=0.300000
     CollisionRadius=120.000000
     CollisionHeight=120.000000
}
