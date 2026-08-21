//=============================================================================
// PatrolPoint.
//=============================================================================
class PatrolPoint extends NavigationPoint
	native;

#exec Texture Import File=Textures\Pathnode.pcx Name=S_Patrol Mips=Off Flags=2

//var() name      Nextpatrol; //next point to go to
var() name      NextPatrol_ObjectName; //Same as NextPatrol, but you can use the name of the object instead of the Tag
var() float     pausetime; //how long to pause here
var() bool      bUseLookDir; //If on, and pausetime is positive, char will turn to this point's orientation during the pause.
var	 vector     lookdir; //direction to look while stopped
var() name      PatrolAnim;
var() name      PauseAnim;
var() sound     PatrolSound;
var() byte      numAnims;
var int	        AnimCount;
var() bool      bLeadActorWaitPoint;  //When leading and actor, say harry, this is a point that the char can stop at, and turn to face the actor he's leading.
var PatrolPoint PrevPatrolPoint;
var PatrolPoint NextPatrolPoint;
var() name      EventToSend;
var() name      ActionKeyword; //Calls the pawn's matching action event when this point is reached.
var() bool      bDestroyPawn;
var() bool      bUseOrientationForSplineTan;

var() bool      bStartOfUnlinkedChain; //Uses PatrolPointLinkTag to link up the chain, starting with this one.
var() name      PatrolPointLinkTag; //Common tag to link patrolpoints together

var   vector    vFraySplineTangent;
var   bool      bHasSplineInfo;
var   float     fTanLenIn;  //If this is zero, first call to GetTanLenIn() sets it based on PrevPatrolPoint.
var   float     fTanLenOut;

//var   vector    vFraySplineTangentNormal;  //normalized version

//Damn, these should go in a derived class, but the level is already built, so I hacked 'em in here.
var()  float    fJumpHorizSpeed;
var()  float    fJumpVertSpeed;
var()  float    fJumpAnimMultiplier;

var()  bool     bStopBossEncounter;

//*************************************************************************************************
function PreBeginPlay()
{
	local PatrolPoint a;

	if (pausetime > 0.0)
		lookdir = 200 * vector(Rotation);

	//If our NextPatrolPoint hasn't been set, set it
	if( NextPatrolPoint == none )
		InitNextPatrolPoint();

	//If we found a NextPatrolPoint, let that NextPatrolPoint know that we're it's prevPatrolPoint
	if( NextPatrolPoint != none )
		NextPatrolPoint.PrevPatrolPoint = self;

	//If we have no PrevPatrolPoint, look for one.  For the self linking chain, this'll run on the first point, but that's ok, it wont find anything.
	if( PrevPatrolPoint == none )
	{
		//This will pretty much set all the patrolpoints' NextPatrolPoints, right now.
		foreach AllActors(class'PatrolPoint', a)
		{
			//If this PatrolPoint doesn't have a NextPatrolPoint, try and init it now
			if( a.NextPatrolPoint == none )
			{
				a.InitNextPatrolPoint();

				//If not successfully set, skip this one
				if( a.NextPatrolPoint == none )
					continue;
			}

			//If it now has a NextPatrolPoint, see if it links to us
			if( a.NextPatrolPoint == self )
			{
				PrevPatrolPoint = a;
				break;
			}
		}
	}

	//Now find our frayspline tangent line
	CalcFraySplineTangent();

	Super.PreBeginPlay();
}

//*************************************************************************************************
function InitNextPatrolPoint()
{
	local PatrolPoint     CurrentPoint;
	//local PatrolPoint     LastPoint;
	local PatrolPoint     tempPatrolPoint;
	local float           fDist, fClosestDist;
	local PatrolPoint     ClosestActor;

	//find the patrol point with the tag specified by Nextpatrol
	if( NextPatrol_ObjectName != '' )
	{
		foreach AllActors(class 'PatrolPoint', NextPatrolPoint)
			if( NextPatrolPoint.name == NextPatrol_ObjectName )
				break; 
	}
	else //This is unlinked, 
	{
		//Ok, if this is the start of a link chain, and it has a link tag, and it's not linked right now,
		// link the whole damn thing up.
		if(   bStartOfUnlinkedChain
		   && PatrolPointLinkTag != ''
		   && NextPatrolPoint == none
		  )
		{
			CurrentPoint = self;

			while(true)
			{
				//Find the closest PatrolPoint to the current patrolpoint, with the same link tag, that doesn't already have a NextPatrolPoint set

				fClosestDist = 100000000;
				ClosestActor = none;

				//Hope this isn't too slow doing this...
				foreach AllActors(class'PatrolPoint', tempPatrolPoint)
				{
					//Skip this one, if the tags dont match,   or if it's already been set
					if(   tempPatrolPoint.PatrolPointLinkTag != PatrolPointLinkTag
					   || tempPatrolPoint.NextPatrolPoint != none
					  )
						continue;

					fDist = VSize( CurrentPoint.Location - tempPatrolPoint.Location );

					if( fDist < fClosestDist )
					{
						fClosestDist = fDist;
						ClosestActor = tempPatrolPoint;
					}
				}

				//If we didnt' find a new actor, we're done
				if( ClosestActor == none )
					break;

				//otherwise link up
				CurrentPoint.NextPatrolPoint = ClosestActor;
				ClosestActor.PrevPatrolPoint = CurrentPoint;

				CurrentPoint = ClosestActor;
			}

			//All right, the whole chain should be linked up now.
		}
		// turn this on, and finish, if splines are needed for unlinked chains
		//else //Same as previous if, but not the chain head, just some unlinked node
		//if(   !bStartOfUnlinkedChain
		//   && PatrolPointLinkTag !=''
		//   && NextPatrolPoint == none
		//  )
		//{
		//	//Find the bStartOfUnlinkedChain with the same tag, and call InitNextPatrolPoint on it.  This will link them up before any other
		//	// nodes finish their PreBeginPlay, which will make the spline tangents correct.
		//}
	}
}

//*************************************************************************************************
function CalcFraySplineTangent()
{
	//First, special cases
	if( PrevPatrolPoint == none && NextPatrolPoint == none )
		return;

	bHasSplineInfo = true;

	//If no prev point
	if( PrevPatrolPoint == none )
	{
		//This should actually do the circle calc with the next 3 points
		vFraySplineTangent = NextPatrolPoint.Location - Location;
		fTanLenOut = vsize(vFraySplineTangent);

		if( bUseOrientationForSplineTan )
			vFraySplineTangent = vector( Rotation );
		else
			vFraySplineTangent = normal(vFraySplineTangent);

		fTanLenIn = fTanLenOut;
	}
	else
	if( NextPatrolPoint == none )
	{
		vFraySplineTangent = Location - PrevPatrolPoint.Location;
		//fTanLenIn = vsize(vFraySplineTangent);
		fTanLenOut = vsize(vFraySplineTangent);

		if( bUseOrientationForSplineTan )
			vFraySplineTangent = vector( Rotation );
		else
			vFraySplineTangent = normal(vFraySplineTangent);
	}
	else  //prev and next available
	{
		//CalcTangentFrom3Points( PrevPatrolPoint, self, NextPatrolPoint, 2 );  //2 which param you're putting the tangent on


		fTanLenOut = vsize(NextPatrolPoint.Location - Location);

		if( bUseOrientationForSplineTan )
			vFraySplineTangent = vector( Rotation );
		else
			vFraySplineTangent = normal(NextPatrolPoint.Location - PrevPatrolPoint.Location);

		//Gets set in first call to GetTanLenIn()
		//fTanLenIn = vsize(vFraySplineTangent);
	}

}

//*************************************************************************************************
function float GetTanLenIn()
{
	if( fTanLenIn == 0 )
		fTanLenIn = PrevPatrolPoint.GetTanLenOut();

	return fTanLenIn;
}

//*************************************************************************************************
function float GetTanLenOut()
{
	return fTanLenOut;
}

//*************************************************************************************************
// TangentActor is which of the 3 parameters you're putting the tangent result on

//This works, but is way overkill.  oops.
//
//function CalcTangentFrom3Points( actor a1, actor a2, actor a3, int TangetActor )
//{
//	local vector  v1;
//	local float   a;
//	local vector  v2;
//	local float   L;
//	local vector  vB;
//	local float   vBLen;
//	local vector  v1crossv2;
//	local float   Cosv1v2;
//	local vector  vCircleCenter;
//	
//	//Make a vector from a1 to the midpoint between a1 and a2
//	v1 = (a2.Location - a1.Location) * 0.5;
//
//	//let a be the length of that
//	a = VSize( v1 );
//
//	if( a == 0 )
//	{
//		vFraySplineTangent = a3.Location - a2.Location;
//		return;
//	}
//
//	//Let v2 be a vector from that midpoint to a3
//	v2 = a3.Location - (a1.Location + v1);
//
//	//Let L be the length of that
//	L = VSize( v2 );
//
//	if( L == 0 )
//	{
//		vFraySplineTangent = a2.Location - a1.Location;
//		return;
//	}
//
//	//Get our perp vector vB (perp to v1, and in the plane), v1 x (v1 x v2)    this vector of course points towards the center of the circle
//	v1crossv2 = v1 cross v2;
//
//	//If the three points are in a straight line, just return line from a2 to a3
//	if( v1crossv2.x == 0 && v1crossv2.y == 0 && v1crossv2.z == 0 )
//	{
//		vFraySplineTangent = a3.Location - a2.Location;
//		return;
//	}
//
//	vB = v1 cross v1crossv2;
//
//	//Get the cosine of the angle between vB and v2
//	Cosv1v2 = normal(vB) dot normal(v2);
//
//	//We now have enough info to find then length of vB (the normal from a1+v1, heading towards the center of the circle)
//	vBLen = (a*a - L*L) / (-2.0 * L * Cosv1v2);
//
//	//rescale vB to be length vBLen
//	vB = normal(vB) * vBLen;
//
//	//Now we know where the center of the circle is, calc our tangent line.  it's just the radius cross the plane normal
//	vCircleCenter = a1.Location + v1 + vB;
//	vFraySplineTangent = (a2.Location - vCircleCenter) cross v1crossv2;
//
//	//normalize it , and set it to the distance between the two points.
//	vFraySplineTangent = normal(vFraySplineTangent) * vsize(a3.Location - a2.Location);
//
//	//maybe scale it by the amount of curvature
//	//vFraySplineTangent *= 1 + d/(2*vsize(a1.Location-vCircleCenter))*(Pi/2 - 1);
//	
//}

//*************************************************************************************************

defaultproperties
{
     bDirectional=True
     SoundVolume=128
	 Texture=S_Patrol
     CollisionRadius=+00020.000000
     CollisionHeight=+00020.000000
}
