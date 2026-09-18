//=============================================================================
// A actor pawn that doesn't display any mesh, 
// used to attach particles or use other actor properties minus a mesh
//=============================================================================


class HiddenHPawn expands HPawn;

#exec Texture Import File=Textures\Hidpawn.pcx Name=HiddenPawn Mips=Off Flags=2

var bool bShowHiddenPawns;

function PreBeginPlay()
{
	super.PreBeginPlay();

	//Is this needed?   YES!! It is!  Just putting the four defaults down in defaultproperties WONT make the actor
	// move normally through the world with SetLocation().  You have to call SetCollision to make the actor be TRULY non colliding.
	SetCollision(,,);
	bCollideWorld = false;

	if( bShowHiddenPawns )
	{
		bHidden=false;
		DrawType=DT_Sprite;
		Style=STY_Normal;
	}
}

defaultproperties
{
     bHidden=true
     DrawType=DT_Sprite
     Texture=Texture'HGame.HiddenPawn'
     Mesh=None

	bShowHiddenPawns=false

     bCollideActors=False
     bCollideWorld=False
     bBlockActors=False
     bBlockPlayers=False
	 bBlockCamera=false
	 CollisionRadius=2
	 CollisionHeight=2
}
