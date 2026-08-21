//=============================================================================
// Blocks the camera from passing.
//=============================================================================
class BlockCamera extends Keypoint;

defaultproperties
{
	CollideType=CT_Box
	CollisionRadius=128
	CollisionWidth=128
	CollisionHeight=128

    bCollideActors=True
    bBlockActors=False
    bBlockPlayers=False
}
