//=============================================================================
// Fireball that shoots from Griffindor sword
//=============================================================================
class SwordFireball expands particlefx;

defaultproperties
{
     SourceWidth=(Base=5,Rand=3)
     SourceHeight=(Base=5,Rand=3)
     SourceDepth=(Base=5,Rand=3)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=0)
     Lifetime=(Base=0.5,Rand=0.2)
     ColorStart=(Base=(G=121),Rand=(R=254,G=155,B=7))
     ColorEnd=(Base=(R=249,G=255),Rand=(R=255,G=187,B=47))
     SizeWidth=(Base=16,Rand=8)
     SizeLength=(Base=16,Rand=8)
     SizeEndScale=(Base=2,Rand=2)
     SpinRate=(Base=-3,Rand=6)
     bSystemRelative=True
     Chaos=1
     Textures(0)=Texture'HPParticle.particle_fx.ember00'
}
