//===============================================================================
// A subclass of HAlohomora in Hprops 
//===============================================================================

class HAlohomora extends hprop;

// --------------------------------------------------------------------------------------------
// *** Variables

var ParticleFX			fxExplode;			// ParticleFX for when a Diffindo obj has exploded
var() class<ParticleFX>	fxExplodeClass;		// class that the explodeFX is.


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PreBeginPlay()
{
	// Find harry and save him
	playerHarry = Harry(Level.playerHarryActor);
}

event Destroyed()
{	
	// make sure our fx is shutdown
	if( fxExplode != None )
		fxExplode.Shutdown();
	
	Super.Destroyed();
}

function bool HandleSpellAlohomora( optional baseSpell spell, optional vector vHitLocation )
{
	OnAlohomoraExplode();
	return true;
}

function OnAlohomoraExplode()
{
	// Send out an event that we have been destroyied
	TriggerEvent( event, none, none );

	// Destroy our diffindo object
	Destroy();
}


defaultproperties
{
	// --- AlohomoraObj
	fxExplodeClass=class'aloh_hit'
	
	// --- Character
	eVulnerableToSpell=SPELL_Alohomora
	
	CollisionRadius=10
	CollisionHeight=10
}