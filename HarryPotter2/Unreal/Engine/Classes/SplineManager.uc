
class SplineManager expands InterpolationManager;

//******************************************************************
auto state patrolFollowSpline
{
	function Tick( float dtime )
	{
		Pawn(Owner).SplineTick( dtime );
	}

	//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
//  Begin:
//	FollowSplinePath( SplinePathName, SplineSpeed, 0, FirstSplinePoint, DestSplinePoint );
//  Begin_ParmsSet:
}
