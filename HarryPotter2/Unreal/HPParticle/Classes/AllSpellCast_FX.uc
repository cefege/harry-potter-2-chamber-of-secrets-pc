//=============================================================================
// Dummy class to place all the spell casting FX in the game
//=============================================================================
class AllSpellCast_FX expands ParticleFX;

event FellOutOfWorld()
{
	// over ride the actors version so we don't destroy ourselves when we fall out of the world
}