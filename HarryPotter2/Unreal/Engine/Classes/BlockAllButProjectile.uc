//=============================================================================
// Blocks all actors from passing.
//=============================================================================
class BlockAllButProjectile extends Keypoint;

defaultproperties
{
     bCollideActors=True
     bBlockActors=True
     bBlockPlayers=True
     bDoNotBlockProjectiles=True
}
