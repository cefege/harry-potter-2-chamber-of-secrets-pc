//=============================================================================
// Halo  -- A sprite meant to look like a halo when attached to another object
//=============================================================================
class Halo extends Sprite;

// Note: The halo does not automatically destruct when the owner dies;
// provisions should be made for this externally (usually in the
// owner's Destroyed function).

//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
	// Set up halo to follow it's owner
	if ( Owner != None )
	{
		DrawScale *= Owner.DrawScale;
		SetPhysics( PHYS_Trailer );
		bTrailerPrePivot = false;

		// Unreal hack that offsets trailer along owner's heading to correct for lag
//		bTrailerSameRotation = true;		// *** dpl: Not needed anymore; ParentTick fixes lag...
//		Mass=-11.0;							// *** 
	}
}

function TakeDamage( int Damage, Pawn InstigatedBy, Vector HitLocation, 
					 Vector Momentum, name DamageType )
{
	// You're average Halo pawn doesn't want to take damage directly;
	// Rely on the owner to handle damage managment.
}

defaultproperties
{
}
