//===============================================================================
//  [Boeing747] 
//===============================================================================

class Boeing747 extends HProp;

var() name pathName;
var() float pathSpeed;

var vector OneStartCP;
var vector OneEndCP;
var vector OneLocation;

var vector TwoStartCP;
var vector TwoEndCP;
var vector TwoLocation;

var vector ThreeStartCP;
var vector ThreeEndCP;
var vector ThreeLocation;

var InterpolationManager		IM;
var DynamicInterpolationPoint	TransPath[3];	// Transitional path to follow to get to main path
var int							iTransPoint;	// Position Id of point on main path that player is transitioning to


// Receive this event when the object reaches an interpolation point
function OnEvent(name EventName)
{

	if( EventName == 'ActionDone' )
	{
		playerHarry.clientMessage("ACTION DONE");
	}	
}


function bool StartTransPath()
{
	local float					fDistance;
	local InterpolationPoint	i;
	local vector				X,Y,Z;

	local float					fClosestDistance;
	local InterpolationPoint	ClosestPoint;
	local int					iClosestPoint;

	local float					fTransDistance;
	local InterpolationPoint	TransPoint;



	// Find the specified point on path
	foreach AllActors( class'InterpolationPoint', i, pathName )
	{
		if ( i.Position == 0 )
		{
			TransPoint = i;
			fTransDistance = fDistance;
			iTransPoint = i.Position;
			break;
		}
	}


	if ( TransPoint == None )
	{
		Log( "BroomHarry could not find an interpolation point on path to go to" );
		return false;
	}

	// Create a transition path to desired path...
	// Set-up interpolation point at end of return path
	if ( TransPath[1] == None )
	{
		TransPath[1] = Spawn( class'DynamicInterpolationPoint' );
		TransPath[1].Tag = TransPath[1].Name;
		TransPath[1].Position = 1;
		TransPath[1].bEndOfPath = true;
	}
	TransPath[1].SetLocation( TransPoint.Location );
	TransPath[1].SetRotation( TransPoint.Rotation );
//	TransPath[1].DesiredSpeed = TransPoint.DesiredSpeed;
	TransPath[1].DesiredSpeed = 1200;
	TransPath[1].StartControlPoint = OneStartCP;
	TransPath[1].EndControlPoint = OneEndCP;

	// Set-up interpolation point at beginning of return path
	if ( TransPath[0] == None )
	{
		TransPath[0] = Spawn( class'DynamicInterpolationPoint', , TransPath[1].Tag );
		TransPath[0].Position = 0;
		TransPath[0].bEndOfPath = false;
		TransPath[0].Next = TransPath[1];
		TransPath[0].Prev = TransPath[1];
		TransPath[1].Next = TransPath[0];
		TransPath[1].Prev = TransPath[0];
	}

	TransPath[0].SetLocation( Location );
//	TransPath[0].SetRotation( Rotation );
	TransPath[0].DesiredSpeed = VSize( Velocity );

	GetAxes( Rotation, X, Y, Z );
	TransPath[0].StartControlPoint =  TwoStartCP;
	TransPath[0].EndControlPoint   = TwoEndCP;

	// Start flying on transition path;
	SetCollision( true, false, false );
	bCollideWorld = false;
	bInterpolating = true;
	SetPhysics( PHYS_None );

	IM = Spawn( class'InterpolationManager', self );
	IM.Init( TransPath[1], 1.0, false );

	return true;


}

function DestroyTransPath()
{
	local InterpolationManager	IM_ToStop;

	if ( IM != None )
	{
		IM_ToStop = IM;
		IM = None;		// Tells FinishInterpolation event that path ended early
		IM_ToStop.FinishedInterpolation( None );
	}

	TransPath[0].Destroy();
	TransPath[1].Destroy();


}
		

function StartOnPath()
{
	FollowSplinePath( pathName,		//optional name  PathTagName
					pathSpeed,		//optional float speed
					0 ,				//optional float accel
					,				//optional name  StartPointName
					 				//optional name  EndPointName
		 			);

}



defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skBoeing747Mesh'
     DrawScale=6
     AmbientGlow=50
     CollisionRadius=1000
     CollisionHeight=250

	 pathName=747Path
	 pathSpeed=1200

	OneStartCP=(3785.495,-994.6182,0)
	OneEndCP=(-3785.495,994.6182,0)
	OneLocation=(-5659.978,-13311.95,-2470)

	TwoStartCP=(648.5029,-2562.644,737.5413)
	TwoEndCP=(-648.5029, 2562.644, -737.5413)
	TwoLocation=(750.5319,-16052.91,-255)

	ThreeStartCP=(-1949.424,-364.2012,0)
	ThreeEndCP=(1949.424, 364.2012, 0)
	ThreeLocation=(-1749.859,-20907.12,665)
}
