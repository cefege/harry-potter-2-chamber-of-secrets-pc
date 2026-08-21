
//We assume our owner is an HPawn

class GenericColObj expands HiddenHPawn;

var  bool  bIsHead;  //This obj is used all down the snake, but this sais it's the head.  An ENUM would be better.

//***************************************************************
function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	return HPawn(Owner).HandleSpellFlipendo( spell, vHitLocation );
}

//***************************************************************
function touch( actor other )
{
	if( HPawn(owner) != none )
		HPawn(owner).ColObjTouch( other, self );
}

//***************************************************************
defaultproperties
{
	Mesh=none
	//SkeletalMesh'HarryPotter.skvenomous2Mesh'
	DrawType=DT_None

	CollisionRadius=20
	CollisionHeight=30

	bCollideActors=true

	eVulnerableToSpell=none
	//SPELL_Flipendo

	ShadowClass=none

}
