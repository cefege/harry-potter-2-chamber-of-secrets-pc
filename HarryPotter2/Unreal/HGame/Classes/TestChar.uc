
class TestChar expands HChar;

var() name Start[3];
var() name End[3];
var   float MyTimer;
var   int  CurrentPiece;

function OnEvent(name EventName)
{
	if( EventName == 'ActionDone' )
	{
		playerHarry.ClientMessage("ActionDone");
		//GotoState( 'DoNothing' );

		if( IsInState( 'patrolFollowSpline' ) )
		{
			GotoState( 'DoSplineFollowWait' );
			//FollowNextSplinePath();
		}
	}
}

function FollowNextSplinePath()
{
	CurrentPiece++;
	if( CurrentPiece >= 3 )
		CurrentPiece = 0;

	//FollowSplinePath( 'IPointSet1', 60, 30, 'InterpolationPoint2', 'InterpolationPoint4', [MoveType] MOVE_TYPE_EASE_TO );
	FollowSplinePath( 'InterpolationPointSet3', 60, 30, Start[CurrentPiece], End[CurrentPiece], [MoveType] MOVE_TYPE_EASE_FROM_AND_TO );

	//GotoState('DoSplineFollowWait');
}

auto state() DoNothing
{
  Begin:
	FollowSplinePath( 'InterpolationPointSet3', 60,		 );
	

//  Begin:
	//SetPhysics( PHYS_None );
	/*
	FollowSplinePathAttach( SplinePathName, 50, 0 );

	Sleep(1);
	DetachFromActor();
	Sleep(1);
	ReattachToActor();
	Sleep(1);
	DetachFromActor();
	Sleep(1);
	ReattachToActor();
	Sleep(1);
	DetachFromActor();
	Sleep(1);
	ReattachToActor();
	Sleep(1);
	DetachFromActor();
	Sleep(1);
	ReattachToActor();
	Sleep(1);
	DetachFromActor();
	Sleep(1);
	ReattachToActor();
	*/
}

state patrolFollowSpline
{
	function Tick(float dtime)
	{
		MyTimer += dtime;

		if( MyTimer >= 10 )
		{
			MyTimer = 0;
			//Do some stuff
			GotoState( 'sldkfj' );
		}
	}

	function Timer()
	{
		
	}

	Begin:
		//SetTimer( 3 );

		
}

state() DoSplineFollow
{
  Begin:
	//SetPhysics( PHYS_None );
	//Sleep( 2 );
	CurrentPiece = -1;
	FollowNextSplinePath();

  DoNothing:
	//FollowSplinePath( 'IPointSet1', 60, 30, 'InterpolationPoint2', 'InterpolationPoint4', [MoveType] MOVE_TYPE_EASE_TO );
}

state() stateDoMoveTo
{
  Begin:
	Sleep(2);
	CutCommand( "FlyTo baseCam EaseBetween Fixed x=-12.34 time=1.5" );
}

state DoSplineFollowWait
{
  Begin:
	Sleep( 2 );
	FollowNextSplinePath();
}

state stateGotoPatrol
{
  Begin:
	GotoState('patrol');
}

state stateMoveAround
{
  Begin:
	MoveTo( Location + normal(VRand()*vec(1,1,0)) * 150 );
	Goto 'Begin';
}

defaultproperties
{
	Mesh=SkeletalMesh'HProps.skChristmasTreeMesh'
}
