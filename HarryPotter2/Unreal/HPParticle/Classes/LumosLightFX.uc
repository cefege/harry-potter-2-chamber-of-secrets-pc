//=============================================================================
// Lumos Light Particle fx
//=============================================================================
class LumosLightFX expands ParticleFX;

event FellOutOfWorld()
{
	// over ride the actors version so we don't destroy ourselves
}

defaultproperties
{
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=0)
     ColorStart=(Base=(G=231,B=62))
     ColorEnd=(Base=(G=177,B=100))
     SizeWidth=(Base=36)
     SizeLength=(Base=36)
     SpinRate=(Base=0.5)
     bSystemRelative=True
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}
