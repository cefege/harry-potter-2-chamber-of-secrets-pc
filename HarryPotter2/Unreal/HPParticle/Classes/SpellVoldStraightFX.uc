//=============================================================================

class SpellVoldStraightFX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=180)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     Speed=(Base=100)
     ColorStart=(Base=(R=0,G=255,B=0),Rand=(R=255,G=255,B=255))
     ColorEnd=(Base=(R=128),Rand=(G=64))
     SizeWidth=(Base=4,Rand=20)
     SizeLength=(Base=4,Rand=20)
     SizeEndScale=(Base=0,Rand=7)
     SpinRate=(Base=-6,Rand=12)
     Chaos=1
     Attraction=(X=-1,Y=-1,Z=-1)
     Damping=1.5
     GravityModifier=-0.1
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_VoldSpell'
}
