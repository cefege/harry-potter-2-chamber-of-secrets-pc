//=============================================================================
// GestureSprite
//=============================================================================
class GestureSprite expands Sprite;


function PreBeginPlay()
{
	super.PreBeginPlay();

	//Is this needed?   YES!! It is!  Just putting the four defaults down in defaultproperties WONT make the actor
	// move normally through the world with SetLocation().  You have to call SetCollision to make the actor be TRULY non colliding.
	SetCollision(,,);
	bCollideWorld = false;
}

defaultproperties
{
	// set the texture
	Texture=None

	bHidden=True
	DrawType=DT_Sprite

	Mesh=None
	bCollideWhenPlacing=false
	bCollideActors=false
	bCollideWorld=false
	bBlockActors=false
	bBlockPlayers=false
	CollisionRadius=2
	CollisionHeight=2
}
