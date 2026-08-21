
class HPawn expands Pawn;

//*************************************************************************************
//var                HarryPawn    playerHarry;		// pointer to harry
var                Harry        playerHarry;  //The actual harry player

// support for Despawner
var				   BaseCam 		Camera;
var()			   bool         bDespawnable;
var				   bool         bDespawned;
var				   float		fCurrTime;

var                class<Decal> ShadowClass;
var ()             float        ShadowScale; //defaults to 1.0
var (SpellEffects) bool         bCanLevitate;
var (SpellEffects) float        levHeight;
var bool                        bIsLevitating;
var float                       levDestZ;
var (SpellEffects) bool         bCanTransform;
var (SpellEffects) class<Actor> transformInto;
var bool                        hasTransformed;
var (SpellEffects) class<Actor> classRepairInto;
var (SpellEffects) class<ParticleFX> classRepairParticalFX;
//This will probably work ok for HP2, since objects can be affected by multiple spells.
//var (SpellEffects) bool         bFlintTarget;
//var (SpellEffects) bool         bAvifTarget;
//var (SpellEffects) bool         bVerdTarget;
//var (SpellEffects) bool         bAlohoTarget;
//var (SpellEffects) bool         bFlipTarget;
//var (SpellEffects) bool         bRepairoTarget;
var (SpellEffects) bool         bSpellCausesTrigger;
var                bool         bDoesntDestroySpell; //MirrorInnerBoxObj has this set, so that spellVoldemortStraight doesn't explode when it hits it.
var                bool         bStopLevitating;
var                bool         lockSpell;			// lock the spell casting on until leviatated.
const NUM_ATTACHED_PARTICLE_FX = 2;
var (Display) class<ParticleFX> attachedParticleClass[2];
var (Display) vector            attachedParticleOffset[2];
var ParticleFX                  attachedParticleFX[2];
var					bool		bThrownObjectDamage;   // This object can get damage from a thrown object

var()              bool         bPlayRunAnim;			// if true - play run anim on patrol points, otherwise - walk one.

//nav/patrol stuff
enum enumPatrolType
{
	PATROLTYPE_PATROL_POINTS,
	PATROLTYPE_PATH_SEARCH,
	PATROLTYPE_SPLINE_FOLLOW,
};
var (patrol)       enumPatrolType   ePatrolType;
var                NavigationPoint  navP, tempNavP, LastNavP, NextPathPoint;
var                NavigationPoint  destP;
var (patrol)       name         FirstObjectName;       //First NavigationPoint for path following.  Can be none.
var (patrol)       name         DestinationObjectName; //Destination NavigationPoint for path following. (Or patrolpoint following)
//var (patrol)       int          stationNumber;
//var (patrol)       name         pathType;
var (patrol)       bool         bLoopPath;
//var (patrol)       float        fNavPointColRadius;
var (patrol)       bool         bIgnoreStationRotations;

//var (PatrolPoints) bool         bFollowPatrolPoints;
//var (PatrolPoints) bool       bAutoMoveToNextClosestPoint;
//var (PatrolPoints) name         firstPatrolPointTag;
var(PatrolPoints)  name         firstPatrolPointObjectName;
var(PatrolPoints)  bool         bGoToClosestPatrolPoint;
var(PatrolPoints)  bool         bUseFraySplines;
//var                name         DestPatrolPoint; //If HPawn gets to a point with this name, send 'ActionDone' event.  Use DestinationObjectName
var                bool         bUseFrayMoveTo;
var                name         PatrolPointLinkTag;
var                bool         bSnapToPatrolPoint;

var                bool         bMoveRequest; //Ooof
var                vector       vMoveRequest; //Yet another bastardized variable.  My spline move code sets this to how much it would like the char to move.  Tick then moves it.
var                vector       vLastLocation;//Oof, ouch, oh god.

var                float        LastLevelTime;//saves the last Level.TimeSeconds
var                float        fTimeOnPath;  // 0 to 1
var                bool         bGoBackToLastNavPoint; //If this is on, you'll go back to LastNavP, then turn around and continue normally.
var                PatrolPoint  LastPatrolPoint;  //For automove, dont go back to the last point you were at.

var                actor        LeadingActor;  //for state stateLeadingActor, the actor you are leading.
var(LeadActor)     name         LeadAnim;
var                string       LeadSpeechBumpSet;  //index into LeadSpeechSnd[]
//var(LeadActor)     sound        LeadSpeechSnd[4];

/*
var                SplineManager  SplineManager;
var(Spline)        name         SplinePathName;
var(Spline)        name         FirstSplinePoint;
var(Spline)        name         DestSplinePoint;
var(Spline)        float        SplineSpeed;
var(Spline)        float        SplineAccel; //for ease from/ease to
var(Spline)        enumMoveType eSplineMoveType;
var                InterpolationPoint LastIPoint; //Really just for saving the reference
var                rotator      SplineRotSave;
*/

//   FlyTo vars
var                FlyToController _FlyToController;
var                enumMoveType eFlyMoveType;
var                vector       vFlyToStart;
var                vector       vFlyToDest;
var                vector       vFlyToDestOffset; //for when aFlyToActor is !none
var                float        fFlyToTime;     //Current Time
var                float        fFlyToTimeSpan; //Time for whole travel
var                float        fEaseBetweenLinearness;
var                bool         bFlyToFixedToDestActor; //if true, offset uses world axes, if false(relative), uses actor axes
var                actor        aFlyToActor;
var                bool         bFlyToStayLockedToActor; //stays locked to actor after the flyto.


var()              bool         bObjectCanBePickedUp;
var()              float        fThrowVelocity;
var()              name         ObjectPickupState;

var()              bool         bAccurateThrowing;

var()              bool         bCantStandOnMe;

var()              bool         bIgnoreZonePainDamage;

//************************************************************************************************************
function PreBeginPlay()
{
	Super.PreBeginPlay();

	//foreach AllActors(class'harry', playerHarry)
	//			break;
	playerHarry = harry(level.playerHarryActor);

	if ( playerHarry == None )
		log("No Harry in Map!");

	DesiredRotation.Yaw = Rotation.Yaw;

	if( ShadowClass != none )
	{
		Shadow = Spawn(ShadowClass,self);   // ShadowScale gets "added" in the shadowclass code
		if( ActorShadow(Shadow) != none )
			ActorShadow(Shadow).ShadowSizeFactor *= ShadowScale;
	}

	CreateAttachedParticleFX();
}

//************************************************************************************************************
function PostBeginPlay()
{
	Super.PostBeginPlay();

	if( CutName == "" )
		CutName = string( Name );

	// Find the camera
	foreach AllActors( class'BaseCam', Camera )
		break;
}

//************************************************************************************************************
function TakeDamage( int Damage, Pawn instigatedBy, Vector hitlocation, 
						Vector momentum, name damageType)
{
	//playerHarry.ClientMessage("***** "$self$" take damage:"$damage$" Health:"$Health);

	//Damn!  dont call super.  Default Pawn behaiviour takes off health and then destroys the actor!
	//super.TakeDamage(Damage, instigatedBy, hitlocation, momentum, damageType);

	if( DamageType == 'ZonePain'  &&  !bIgnoreZonePainDamage )
		Destroy();
}

//************************************************************************************************************
function bool PawnCantStandOnMe()
{
	return bCantStandOnMe;
}

//************************************************************************************************************
// This function will be called by the engine when it is time to resolve what this actor does
// if it is inside the current GameState or not.
event OnResolveGameState()
{
	// If we are not in the current gamestate then become hidden and have no collision
	if( !bInCurrentGameState )
	{
		// We are NOT in the current state
		bHidden = true;
		SetCollision(false,false,false);
	}
}

//************************************************************************************************************
//This method is called from a GenericColObj that you might have attached to a bone.
function ColObjTouch( actor other, GenericColObj ColObj )
{
}

//************************************************************************************************************
//Normally true, override in certain states or actors to make it so the cut command "Release" doesn't
// play the IdleAnimName anim.  This fixes when an actor is doing a LeadActor, and then gets released, and they lose their run anim.
function bool ShouldPlayIdleOnRelease()
{
	return true;
}

//************************************************************************************************************
event destroyed()
{
	local int i;

	for( i = 0; i < NUM_ATTACHED_PARTICLE_FX; i++ )
		if(attachedParticleFX[i] != None)
			attachedParticleFX[i].Shutdown();

	if( _FlyToController != none )
		_FlyToController.Destroy();

	super.destroyed();
}

//************************************************************************************************************
function CreateAttachedParticleFX()
{
	local int i;

	for( i = 0; i < NUM_ATTACHED_PARTICLE_FX; i++ )
	{
		if( attachedParticleClass[i] != None )
		{
			attachedParticleFX[i] = ParticleFX(FancySpawn(attachedParticleClass[i],self,,location+attachedParticleOffset[i]));
			attachedParticleFX[i].setRotation(attachedParticleClass[i].default.rotation);
			attachedParticleFX[i].setPhysics(PHYS_Trailer);
			if( attachedParticleOffset[i] != vect(0,0,0) )
			{
				attachedParticleFX[i].bTrailerPrePivot = true;
				attachedParticleFX[i].PrePivot = attachedParticleOffset[i];
			}
		}
	}
}

function killAttachedParticleFX(float time)
{
	local int i;

	for( i = 0; i < NUM_ATTACHED_PARTICLE_FX; i++ )
	{
		if(attachedParticleFX[i] != None)
		{
			attachedParticleFX[i].ParticlesPerSec.base = 0;
			attachedParticleFX[i].LifeTime.base = 0;

			if(time == 0.0)
				attachedParticleFX[i].destroy();
			else
				attachedParticleFX[i].lifeSpan = time;
		}
	}
}


//************************************************************************************************************
auto state() stateIdle
{

}

//************************************************************************************************************
state() stateInactive
{
}

//************************************************************************************************************
state() stateInfoPrint
{
	//function Tick(float dtime)
	//{
	//	playerHarry.ClientMessage("phys:"$Physics$" pos:"$Location);
	//}
}

//************************************************************************************************************
  //The problem
function ThrownLanded(vector HitNormal)
{

}

state stateBeingThrown
{
	//"The problem"  comment out the next line and do a normal make.  You got a firkin problem.
	function Landed(vector HitNormal)	{		ThrownLanded(HitNormal);	}

	function Touch(Actor Other)
	{
		playerHarry.Clientmessage("HPawn: Touch:"$Other);
	}

	function Bump(Actor Other)
	{
		playerHarry.Clientmessage("HPawn: Bump:"$Other);
	}

	function HitWall( vector HitNormal, actor HitWall )
	{
		playerHarry.Clientmessage("HPawn: Hitwall:"$HitWall);
	}

  Begin:
	sleep( 5 );
	goto 'Begin';
}

//************************************************************************************************************
//Override this special, called from Harry, function.
function PawnHearHarryNoise()
{
}

//************************************************************************************************************
function Tick(float dtime)
{
	super.Tick( dtime );

	// try to destroy it every second, if we have to
	fCurrTime += dtime;
	if(fCurrTime < 1.0)
		return;

	fCurrTime = 0.0f;

	if(bDespawnable && bDespawned)
	{
		if( !Camera.CameraCanSeeYou(location) )
		{
			Destroy();
		}
	}

//	playerHarry.ClientMessage("****** Actor:"$self.name$" State:"$GetStateName() );
}


//************************************************************************************************************
//************************************************************************************************************
//************************************************************************************************************
//************************************************************************************************************


//************************************************************************************************************

function DestroyControllers()
{
	if( _FlyToController != none )
		_FlyToController.DisableController();

	super.DestroyControllers();
}

// Patrolling functions.  Normally follows NavigationsPoints,
// or, if bFollowPatrolPoints in PatrolPoints is true, follows path of patrol points.
//  Will add spline following support later.

//*************************************************************************************
function bool FollowPatrolPoints(           name  StartPointName
						         , optional name  EndPointName
						         , optional bool  bSnapToStartLoc
                                 , optional float speed
                                 //, optional float accel
						        )
{
	local PatrolPoint dp;

	foreach AllActors( class'PatrolPoint', dp )
		if( dp.name == StartPointName )
			break;

	if( dp == none )
	{
		Log("FollowPatrolPoints: Couldn't find start point");
		return false;
	}

	if( bSnapToStartLoc )
		SetLocation( dp.Location );

	if( speed != 0 )
		GroundSpeed = speed;
	else
	if( Characters(self) != none )
	{
		if(bPlayRunAnim)
			GroundSpeed = GroundRunSpeed;
		else
			GroundSpeed = GroundWalkSpeed;
	}

	firstPatrolPointObjectName = StartPointName;
	DestinationObjectName = EndPointName;
	//bFollowPatrolPoints = true;
	ePatrolType = PATROLTYPE_PATROL_POINTS;
	bGoBackToLastNavPoint = false;
	navP = none;
	tempNavP = none;
	LastNavP = none;
	GotoState('patrol');

	return true;
}

//*************************************************************************************
function bool NavigateToPathNode(           name  StartPointName //This can be none, in which case the closest start point is found.
                                                                 // The tag from the actor named EndPointName will be used for searching.
						         ,          name  EndPointName
						         , optional bool  bSnapToStartLoc
                                 , optional float speed
                                 //, optional float accel
						        )
{
	local NavigationPoint sp;
	local NavigationPoint dp;

	//Find the dest node
	foreach AllActors( class'NavigationPoint', dp )
		if( dp.name == EndPointName )
			break;

	if( dp == none )
	{
		Log("NavigateToPathNode: Couldn't find end point named:"$EndPointName);
		return false;
	}

	if( StartPointName != '' )
	{
		foreach AllActors( class'NavigationPoint', sp )
			if( sp.name == StartPointName )
				break;
	}

	//If still no start point, find closest start point with same tag as EndPointName actor
	if( sp == none )
		sp = FindClosestNavigationPoint( dp.tag );

	if( bSnapToStartLoc )
		SetLocation( dp.Location );

	if( speed != 0 )
		GroundSpeed = speed;
	else
	if( Characters(self) != none )
	{
		if(bPlayRunAnim)
			GroundSpeed = GroundRunSpeed; 
		else
			GroundSpeed = GroundWalkSpeed;
	}

	FirstObjectName = sp.Name;
	DestinationObjectName = EndPointName;
	//bFollowPatrolPoints = false;
	ePatrolType = PATROLTYPE_PATH_SEARCH;
	navP = none;
	tempNavP = none;
	LastNavP = none;
	GotoState('patrol');

	return true;
}

//*************************************************************************************
state() patrol
{
	function bool ShouldPlayIdleOnRelease() { return false; }

	function startup()
	{
		if(	ePatrolType == PATROLTYPE_PATROL_POINTS )
		{
			//Only look for first patrol tag if navP isn't set.  This lets you go to another state, and then back here
			// again without starting the path over again.
			if( navP == none )
			{
				foreach allActors(class 'navigationPoint',navP)
					if( navP.name == firstPatrolPointObjectName )
						break;

				if( navP == none )
				{
					//playerHarry.ClientMessage("*** HPawn:"$self.Name$" couldn't find navigationPoint:"$firstPatrolPointObjectName);
					GotoState( 'stateIdle' );
					return;
				}

				PatrolPointLinkTag = PatrolPoint(navP).PatrolPointLinkTag;
				LastNavP = navP;
			}
		}
		else
		{
			if(DestinationObjectName != '')
			{
				foreach allActors(class 'navigationPoint',navP)
				{
					destP = navP;

					if(destP != None  &&  destP.Name == DestinationObjectName)
						break;
				}
			}

			if( FirstObjectName != '' )
			{
				foreach allActors(class 'navigationPoint',navP)
					if(navp.Name == FirstObjectName)
						break;
			}
		}
	}

	function EndState()
	{
		LastLevelTime = 0;
		bMoveRequest = false;
		bNoZoneFriction = false;
	}

	function Tick(float dtime)
	{
		Global.Tick(dtime);

		//If bMoveRequest is true, Do a move smooth the amount set in vMoveRequest.  No one should be using this except my bastardized spline stuff.
		// Dont get me wrong, this needs to be fixed, real bad!
		if( bMoveRequest )
		{
			bMoveRequest = false;
			vLastLocation = Location;
			MoveSmooth( vMoveRequest );
		}
	}

  Begin:
	enable( 'Tick' );

	if( ePatrolType == PATROLTYPE_SPLINE_FOLLOW )
		GotoState( 'patrolFollowSpline' );

	startup();
	//playerHarry.clientMessage(self $" starting patrol");
	//Log("*********** char:"$self$" starting patrol");

	bNoZoneFriction = true;

	if( ePatrolType == PATROLTYPE_PATH_SEARCH  &&  FirstObjectName == '' )
		goto 'idleloop';

	if(bPlayRunAnim)
		patrolPlayRunAnim();
	else
		patrolPlayWalkAnim();

  moveLoop:
	if( ePatrolType == PATROLTYPE_PATH_SEARCH )
		NextPathPoint = findPath( navP, DestinationObjectName );

	if(    ePatrolType == PATROLTYPE_PATROL_POINTS
	   &&  bUseFraySplines
	   && !bGoBackToLastNavPoint
	   &&  PatrolPoint(navP).bHasSplineInfo
	  )
	{
		while( !MoveTo_FraySpline() )
			sleep(0.005);
	}
	else //normal
	{
		//If bGoBackToLastNavPoint is set, set navp to LastNavP
		if( ePatrolType == PATROLTYPE_PATROL_POINTS  &&  bGoBackToLastNavPoint )
		{
			navP = LastNavP;
			bGoBackToLastNavPoint = false;
		}

		if( ePatrolType == PATROLTYPE_PATROL_POINTS  &&  bUseFrayMoveTo )
		{
			while( !MoveTo_Fray() )
				sleep(0.005);
		}
		else
		{
			//Only do this if you're NOT right next to the point you're trying to move to.  If you're standing on the exact point, he turns to face yaw=0.  That looks bad.
			if( VSize2d(navP.location - Location) > 1 )
				moveTo(navP.location);
		}

		//if( VSize2d(navP.location - Location) > 10  &&  bSnapToPatrolPoint )
		//	SetLocation2( navP.location );
	}

	if( ePatrolType == PATROLTYPE_PATROL_POINTS )
	{
		_PawnAtPatrolPoint( PatrolPoint(navP) );
	}
	else
	{
		if( navP == destp )
			PawnAtDestination();

		PawnAtStation();
	}

	if( ePatrolType == PATROLTYPE_PATROL_POINTS )
	{
		tempNavP = navP;

		if( PatrolPoint(navP).NextPatrolPoint == none )
		{
			if(bGoToClosestPatrolPoint)
				navP = FindClosestPatrolPoint( LastNavP, navP );  //Find closest, excluding Last one you were at, and the one you're currently at.
			else
				navP = none; //FindClosestPatrolPoint( LastNavP, navP );  //Find closest, excluding Last one you were at, and the one you're currently at.
		}
		else
			navP = PatrolPoint(navP).NextPatrolPoint;

		LastNavP = tempNavP;

		_PostPawnAtPatrolPoint( PatrolPoint(LastNavP), PatrolPoint(navP) );
	}
	else
	{
		navP = NextPathPoint;
	}

	if(navP == none)
	{
  idleLoop:
		while( true )
		{
			loopAnim(idleAnimName, 1.0, 0.75);
			sleep( speechTime );
			speechTime = 0;
			sleep( 0.5 );

			//If loop path, just start the whole patrol process over again by setting navp to FirstObjectName.
			if( bLoopPath )
			{
				if( ePatrolType == PATROLTYPE_PATROL_POINTS )
				{
					foreach allActors(class 'navigationPoint',navP)
						if( navP.name == firstPatrolPointObjectName )
							break;
				}
				else
				{
					foreach allActors(class 'navigationPoint',navP)
						if(navp.Name == FirstObjectName)
							break;
				}

				break; //break the while loop
			}

			//goto 'idleloop';
		}
	}
	
	NextPathPoint = none;

	goto 'moveLoop';
}

//*******************************************************************************************************
state statePatrolPointPause
{
	function bool ShouldPlayIdleOnRelease() { return false; }

  Begin:
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	LoopAnim( PatrolPoint(LastNavP).PauseAnim, 1.0, 0.5 );

	//Actually, use the character's location so that it will always point the same direction.  (the char may not be standing on the patrolpoint)
	if( PatrolPoint(LastNavP).bUseLookDir )
		TurnTo( /*PatrolPoint(LastNavP).*/Location + PatrolPoint(LastNavP).lookdir );

	sleep( PatrolPoint(LastNavP).pauseTime );
	GotoState( 'patrol' );
}

//*******************************************************************************************************
// Try to move to navP, using my fancy new patented "frayspline"
function bool MoveTo_FraySpline()
{
	local float dtime;
	local float d, speed2, fScale;
	local float t;
	local float NewTimeOnPath;

	//local vector vLastNavPoint, vOut;
	local vector v, v1, v2, vMove;
	local PatrolPoint p;

	//All right, my fault.  Any problems with this, and it needs to be changed so it uses tick().
	if( LastLevelTime == 0 )
		LastLevelTime = Level.TimeSeconds;

	dtime = Level.TimeSeconds - LastLevelTime;
	LastLevelTime = Level.TimeSeconds;

	dtime = fmin( dtime, 0.1 );  //10 frame a second max

	//Find what percent we need to move along
	d = GroundSpeed * dtime;
	t = d / PatrolPoint(navP).GetTanLenIn(); //t is set to how much percent we want to advance so we move the distance we require ( d )
	NewTimeOnPath = fTimeOnPath + t;

	if( NewTimeOnPath > 1 )
		NewTimeOnPath = 1;

	//Do a movesmooth towards the current loc
	p = PatrolPoint(navP).PrevPatrolPoint;
	if( p == none )
	{
		//Notice this ignores NewTimeOnPath
		vMove = (navP.Location - Location) * vect(1,1,0);
		if( vMove != vect(0,0,0) )
		{
			if( d < VSize(vMove) ) //Only scale back to normal speed if we're further away from our dest as the step size we're about to do
				vMove = normal(vMove) * d;
		}
	}
	else //we have a PrevPatrolPoint
	{
		v2 = navP.Location + (PatrolPoint(navP).vFraySplineTangent * -PatrolPoint(navP).GetTanLenIn() * (1-NewTimeOnPath));
		v1 = p.Location    + p.vFraySplineTangent * p.GetTanLenOut() * NewTimeOnPath;
		v = v1 + (v2-v1) * EaseBetween(NewTimeOnPath);

		vMove = v - Location;  //this is how much we'd like to move
		vMove.z = 0;

		//vMove = normal(vMove) * GroundSpeed * dtime;

		//Scale it back, based on our ground speed
		speed2 = VSize(vMove) / dtime;
		//playerHarry.ClientMessage("********* speed2="$speed2$" z="$vMove.z/dtime$"  dtime="$dtime);
		if( speed2 > GroundSpeed*1.5 )
			vMove *= GroundSpeed*1.5 / speed2;
	}

	//MoveSmooth( vMove );

	//You'll want to rotate to what the last vMoveRequest was, since that's how much we actually moved.
	//One thing could try here, set DesiredRotation to rotator( Location-vLastLocation ).  Cause that's pointing the direction you actually moved, instead of the direction you requested to move.
	//DesiredRotation = rotator( vMoveRequest * vect(1,1,0) );//SetRotation( rotator(vMove) );
	v1 = Location-vLastLocation;
	if( v1.x != 0 || v1.y != 0 )
		DesiredRotation = rotator( v1 * vect(1,1,0) );

	fTimeOnPath = NewTimeOnPath;

	//Ah, even more bastardization, of everything that is sacred!  Set bMoveRequest to true, and Tick will move the char vMoveRequest.
	bMoveRequest = true;
	vMoveRequest = vMove;

	if( vsize2d(Location - navP.Location) < 1 )
	{
		fTimeOnPath = 0;
		LastLevelTime = 0;
		return true;
	}
	else
	{
		return false;
	}
}

//*******************************************************************************************************
function bool MoveTo_Fray()
{
	local float dtime;
	local float d, speed2, fScale;
	local float t;

	//local vector vLastNavPoint, vOut;
	local vector v, v1, v2, vMove;
	local PatrolPoint p;

	//All right, my fault.  Any problems with this, and it needs to be changed so it uses tick().
	if( LastLevelTime == 0 )
		LastLevelTime = Level.TimeSeconds;

	dtime = Level.TimeSeconds - LastLevelTime;
	LastLevelTime = Level.TimeSeconds;

	dtime = fmin( dtime, 0.1 );  //10 frame a second max

	//Find what percent we need to move along
	d = GroundSpeed * dtime;

	//Do a movesmooth towards the current loc
	vMove = normal( (navP.Location - Location) * vect(1,1,0) ) * d;

	//"Save" our location.  Actually, it was saved in Tick, before the MoveSmooth.
	v = vLastLocation;

	//MoveSmooth( vMove );

	//One thing could try here, set DesiredRotation to rotator( Location-vLastLocation ).  Cause that's pointing the direction you actually moved, instead of the direction you requested to move.
	//DesiredRotation = rotator( vMoveRequest * vect(1,1,0) );//SetRotation( rotator(vMove) );
	v1 = (Location-vLastLocation);
	if( v1.x != 0 || v1.y != 0 )
		DesiredRotation = rotator( v1 * vect(1,1,0) );

	//Ah, even more bastardization, of everything that is sacred!  Set bMoveRequest to true, and Tick will move the char vMoveRequest.
	bMoveRequest = true;
	vMoveRequest = vMove;

	//playerHarry.ClientMessage("********* vsize2d(Location - navP.Location)="$vsize2d(Location - navP.Location));
	//You're there, if you crossed the point
	//if( vsize2d(Location - navP.Location) < 1 )
	v1 = navP.Location -        v;
	v2 = navP.Location - Location;
	v1.z = 0;  //hee hee, if you dont set z to zero and the nav point is off the floor, the vectors point more up than out, so the dot is always positive.
	v2.z = 0;
	t = v1 dot v2;

	if( t <= 0 )
	{
		LastLevelTime = 0;
		return true;
	}
	else
	{
		return false;
	}
}

//*******************************************************************************************************
//You can override this, if you're cool.
function patrolPlayRunAnim()
{
	loopAnim(RunAnimName, , 0.75);
}

//*******************************************************************************************************
function patrolPlayWalkAnim()
{
	loopAnim(WalkAnimName, , 0.75);
}

//*******************************************************************************************************
//Dont consider ExclusionActor in the check
function PatrolPoint FindClosestPatrolPoint( actor ExclusionActor1, actor ExclusionActor2 )
{
	local PatrolPoint     tempPatrolPoint;
	local float           fDist, fClosestDist;
	local PatrolPoint     ClosestActor;

	fClosestDist = 100000;

	//Hope this isn't too slow doing this...

	foreach AllActors(class'PatrolPoint', tempPatrolPoint)
	{
		//If PatrolPointLinkTag is set, only use tempPatrolPoints that have the same name
		if( PatrolPointLinkTag != ''  &&  PatrolPointLinkTag != tempPatrolPoint.PatrolPointLinkTag )
			continue;

		fDist = VSize( Location - tempPatrolPoint.Location );

		if(   tempPatrolPoint != ExclusionActor1
		   && tempPatrolPoint != ExclusionActor2
		   && fDist < fClosestDist
		  )
		{
			fClosestDist = fDist;
			ClosestActor = tempPatrolPoint;
		}
	}

	return ClosestActor;
}

//*******************************************************************************************************
function NavigationPoint FindClosestNavigationPoint( name PathTag )
{
	local NavigationPoint     tempPatrolPoint;
	local float               fDist, fClosestDist;
	local NavigationPoint     ClosestActor;

	fClosestDist = 100000;

	foreach AllActors(class'NavigationPoint', tempPatrolPoint, PathTag)
	{
		fDist = VSize( Location - tempPatrolPoint.Location );

		if( fDist < fClosestDist )
		{
			fClosestDist = fDist;
			ClosestActor = tempPatrolPoint;
		}
	}

	return ClosestActor;
}

//*******************************************************************************************************
//Override this so you can do your own at station stuff
function PawnAtStation()
{
	//gotostate('atstation');
}

//** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** 
function PawnAtDestination()
{
	OnEvent( 'ActionDone' );
}

//*******************************************************************************************************
function _PawnAtPatrolPoint(PatrolPoint pp)
{
	PawnAtPatrolPoint(pp);

	if( pp.EventToSend != '' )
		TriggerEvent( pp.EventToSend, none, self );
}

function PawnAtPatrolPoint(PatrolPoint pp)
{
	if( pp.Name == DestinationObjectName )
		OnEvent( 'ActionDone' );
}

//*******************************************************************************************************
//Here, navP (passed in as NextP) has already been set to the next one you're going to
function _PostPawnAtPatrolPoint(PatrolPoint CurrentP, PatrolPoint NextP)
{
	PostPawnAtPatrolPoint(CurrentP, NextP);

	if( CurrentP.bDestroyPawn )
		Destroy();

	if( CurrentP.pauseTime > 0 )	
		GotoState('statePatrolPointPause');

	if( CurrentP.PatrolSound != none )
		PlaySound( CurrentP.PatrolSound );
}

function PostPawnAtPatrolPoint(PatrolPoint CurrentP, PatrolPoint NextP)
{
}


function DoFlyTo( vector DestLoc, enumMoveType MoveType, float TimeSpan )
{
	DoFlyToSetup();

	aFlyToActor	 =   none;
	eFlyMoveType =   MoveType;
	vFlyToStart =    Location;
	vFlyToDest =     DestLoc;
	fFlyToTimeSpan = TimeSpan; //Time for whole travel
	fFlyToTime =     0;     //Current Time
}

function DoFlyTo_Actor( actor a, vector vOffset, enumMoveType MoveType, float TimeSpan, bool bFixedToChar, bool bStayLockedToActor )
{
	DoFlyToSetup();

	aFlyToActor	 =   a;
	bFlyToFixedToDestActor = bFixedToChar; //if true, offset uses world axes, if false(relative), uses actor axes
	bFlyToStayLockedToActor = bStayLockedToActor;
	eFlyMoveType =   MoveType;
	vFlyToStart =    Location;
	vFlyToDest =     a.Location;  //Mainly for if the actor gets destroyed
	vFlyToDestOffset=vOffset;
	fFlyToTimeSpan = TimeSpan; //Time for whole travel
	fFlyToTime =     0;     //Current Time

	//Since the FlyToController is making this HPawn follow some other actor, set the FlyToController's TickParent to that actor
	// so the actor get's ticked before this or the FlyToController.  This could probably be moved to FlyToController.
	_FlyToController.TickParent = a;
}

function DoFlyToSetup()
{
	//GotoState( 'stateFlyingToLoc' );
	if( _FlyToController == none )
	{
		_FlyToController = FlyToController(FancySpawn( class'FlyToController', self ));
		_FlyToController.SetOwner( self );
	}

	//Set TickParent to the FlyToController so it get's ticked before this HPawn
	TickParent = _FlyToController;

	_FlyToController.EnableController();

	playerHarry.ClientMessage( "DoFlyTo:"$CutNotifyActor$" "$sCutNotifyCue);

	if( _FlyToController == none )
	{
		Log("************* _FlyToController wasnt' made");
		playerHarry.ClientMessage( "*****FLYTO ERROR: _FlyToController wasnt' made" );
	}
}

//**********************************************************************************
//Called from the controller when the flyto is done
function OnFlyToDone()
{
	playerHarry.ClientMessage("Flew to:" $vFlyToDest$"  Location:"$Location$" cue:"$sCutNotifyCue);
	OnEvent( 'ActionDone' );
	//playerHarry.ClientMessage( "***** cccc "$CutNotifyActor$" "$sCutNotifyCue);
	DoCutCueNotify();
}

//**********************************************************************************
//Uses PatrolPoints to lead an actor to a dest PatrolPoint
function LeadActor(actor Other, name StartPatrolPoint, Name EndPatrolPoint, name anim, string in_LeadSpeechBumpSet)
{
	FollowPatrolPoints( StartPatrolPoint, EndPatrolPoint );
	LeadingActor = Other;
	if( anim != '' )
		LeadAnim = anim;
	LeadSpeechBumpSet = in_LeadSpeechBumpSet;
	GotoState( 'stateLeadingActor' );
}

//**********************************************************************************
state stateLeadingActor extends patrol
{
	function bool ShouldPlayIdleOnRelease() { return false; }

	function PostPawnAtPatrolPoint(PatrolPoint CurrentP, PatrolPoint NextP)
	{
		super.PostPawnAtPatrolPoint( CurrentP, NextP );

		//If we're at our dest point, or
		//if this is a Point that we should wait at for our follow actor, goto our lookat actor state.
		// And harry isn't near us or past us already.
		if(   CurrentP.name == DestinationObjectName
		   || CurrentP.bLeadActorWaitPoint   &&   !LeadActor_ShouldMoveToNextPatrolPoint( LeadingActor )
		  )
			GotoState( 'stateLeadingActorPause' );		
	}
}

//*******************************************************************************************************
state stateLeadingActorPause
{
	function Tick( float dtime )
	{
		DesiredRotation.yaw = rotator( LeadingActor.Location - Location ).yaw;
	}

	function bool ShouldPlayIdleOnRelease() { return false; }

	function AnimEnd()
	{
		if( AnimSequence == LeadAnim )
			LoopAnim( IdleAnimName, , 0.75 );
	}

  Begin:
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	if( LeadAnim != '' )
		PlayAnim( LeadAnim, 1.0, 0.5 );
	else
		LoopAnim( IdleAnimName, 1.0, 0.5 );

	do
	{
		//See if our LeadingActor that is following us encroaching on our position, or has already passed us.
		if(   LastNavP.name != DestinationObjectName   //we also have to not be at the last patrolpoint.
		   && LeadActor_ShouldMoveToNextPatrolPoint( LeadingActor )
		  )
		{
			if( HChar(self) != none  &&  LeadSpeechBumpSet != "" )
				HChar(self).DoBumpLine(true, LeadSpeechBumpSet);

			//PlaySound( LeadSpeechSnd[ LeadSpeech ] );
			GotoState( 'stateLeadingActor' );
		}

		sleep( 0.2 );
	}until(false);
}

//************************************************************************************************************
function bool LeadActor_ShouldMoveToNextPatrolPoint( actor other )
{
	local PatrolPoint pp;

	//First try: If other is within a certain distance, return true
	if( VSize( other.Location - Location ) < 300 )
		return true;	
	
	//Second try: Go down this patrolpoint chain and see if other is next to any of the points.
	pp = PatrolPoint(navP);

	do
	{
		if( pp == none )
			break;

		if( VSize(other.Location - pp.Location) < 300 )
			return true;

		pp = pp.NextPatrolPoint;

		//Look for looping patrolpoint path
		if( pp == navP )
			break;
	}until(false);

	return false;
}

//************************************************************************************************************
//************************************************************************************************************
//************************************************************************************************************
//************************************************************************************************************
//************************************************************************************************************

// Called when an HPawn is hit by a thrown object (ie: SpikyPlantSpikes). This function 
// can be overridden by the Hpawn but the base behavior is to call the Spell Handler that the 
// HPawn is vulnerable to. If you override this function you DON'T need to call Super.HitByThrownObject
//
// Thoughts for anyone writing a creature that can be thrown by Harry: 
//		1.	Remember to not call this function when your creature is touched. This could set up
//			a cascade effect and a possible infinite loop.
//		2.	Use ObjectType. This is set up like the call to TakeDamage so you have alot
//			of useful information right here.
//		3.	At this time the default behavior is the same for every thrown object but you
//			may need to seperate it out based on type (so it would be nice if it worked). 
//		4.	By Long and Irritating trial and error it's been found that if you spawn an object
//			you can't get collision detection on anything that is already inside the object.
//			Basically, what this means is that if you throw something at an HPawn and it spawns something
//			right on/very near it you won't get a 'Touch'. Be Aware of this and if things are acting oddly
//			try a foreach. 
function HitByThrownObject( int Damage, HPawn instigatedBy, Vector hitlocation, 
							Vector momentum, name ObjectType )
{


	playerHarry.ClientMessage( Name $ " In HitByThrownObject from: " $ instigatedBy );

	if ( instigatedBy.bThrownObjectDamage == true )
	{
		playerHarry.ClientMessage( " This actor has been damaged by a thrown object " );

		switch ( instigatedBy.eVulnerableToSpell)
		{
		case SPELL_Rictusempra:	HandleSpellRictusempra( none, Hitlocation );	break;
		case SPELL_Alohomora:	HandleSpellAlohomora(	none, HitLocation );	break;
		case SPELL_Diffindo:	HandleSpellDiffindo(	none, HitLocation );	break;
		case SPELL_Ecto:		HandleSpellEcto(		none, HitLocation );	break;
		case SPELL_Flipendo:	HandleSpellFlipendo(	none, HitLocation );	break;
		case SPELL_Lumos:		HandleSpellLumos(		none, HitLocation );	break;
		case SPELL_Rictusempra:	HandleSpellRictusempra( none, HitLocation );	break;
		case SPELL_Skurge:		HandleSpellSkurge(		none, HitLocation );	break;
		case SPELL_Spongify:	HandleSpellSpongify(	none, HitLocation );	break;
		case SPELL_None:
			log("HPawn is vulnerable to SPELL_None. Change eVulnerableToSpell or overwrite HitByThrownObject" );
			break;
		default:
			log("HPawn is not vulnerable to a known spell. Update HPawn's HitByThrownObject OR overwrite HitByThrownObject OR change eVulnerableToSpell");
			break;
		}

	}
}

	


// ********************************
// *** spell creation functions ***
// ********************************
function baseSpell SpawnSpell( class<baseSpell> spellClass, optional actor aTarget )
{
	local baseSpell spell;
	
	// Debug
	playerHarry.ClientMessage("SpawnSpell called with spellClass:" $spellClass $" and Target:" $aTarget );
	
	// if we don't specify what spell we want but we do specify what our target is,
	// then use the spell that the target's eVulnerableToSpell
	if( spellClass == None && aTarget != None && aTarget.eVulnerableToSpell != SPELL_None )
		spellClass = baseWand(playerHarry.weapon).GetClassFromSpellType( aTarget.eVulnerableToSpell );
	
	// Create the spell and save a refrence to it
	spell = spawn( spellClass );
	
	// Setup our spell
	spell.InitSpell( self, aTarget );
	
	return spell;
}

function baseSpell SpawnSpellEx( class<baseSpell> spellClass, vector loc, rotator rot, optional actor aTarget )
{
	local baseSpell spell;
	
	// Create the spell and save a refrence to it
	spell = spawn(spellClass,,,loc,rot);
	
	// Setup our spell
	spell.InitSpell( self, aTarget );
	
	return spell;
}

// ********************************
// *** spell handling functions ***
// ********************************

//Dont override this one, override one of the specific ones that follow.
function OnSpellHit( optional baseSpell spell, optional vector vHitLocation )
{
	if( bSpellCausesTrigger )
	{
		playerHarry.ClientMessage("OnSpellHit: SentTrigger:"$Event);
		TriggerEvent( Event, self, self );
	}
}

// Implementation of these spell functions are usually in the derived class
function bool HandleSpellAlohomora( optional baseSpell spell, optional vector vHitLocation )
{ 
	return false;
}

function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}

function bool HandleSpellEcto( optional baseSpell spell, optional vector vHitLocation )
{ 
	return false; 
}

function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{ 
	return false;
}

function bool HandleSpellLumos( optional baseSpell spell, optional vector vHitLocation )
{ 
	return false;
}

function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}

function bool HandleSpellSkurge( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}

function bool HandleSpellSpongify( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}

// --- Dueling spells

function bool HandleSpellDuelRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}

function bool HandleSpellDuelMimblewimble( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}

function bool HandleSpellDuelExpelliarmus( optional baseSpell spell, optional vector vHitLocation )
{
	return false;
}


// --- Handle Spell Incantation Sound

function HandleSpellIncantationSound( ESpellType SpellType )
{
	local String SpellIncantation;
	local Sound  SpellSound;
	
	switch( SpellType )
	{
		case SPELL_DuelRictusempra:
			SpellSound = sound'HPSounds.Magic_sfx.cast_Rictusempra';
			break;

		case SPELL_DuelMimblewimble:
			SpellSound = sound'HPSounds.Magic_sfx.cast_Mimblewimble';
			break;
			
		case SPELL_DuelExpelliarmus:
			// do not play Expelliarmus sound here, decide what to play later
//			SpellSound = sound'HPSounds.Magic_sfx.cast_Expelliarmus';
			SpellSound = none;
			break;
	}
	
	// Play Incantation Sound
	if( SpellIncantation != "" )
		PlaySound( Sound(DynamicLoadObject("AllDialog."$SpellIncantation, class'Sound')), SLOT_Talk, , true);
	
	// Play Spell SoundFX
	if( SpellSound != None )
		PlaySound( SpellSound, SLOT_None, , true);
}

//***********************************************************************************************************
function OnEvent(name EventName)
{
	if( EventName == 'ActionDone' )
	{

	}
}

//***********************************************************************************************************
state stateDestroy
{
  Begin:
	//Wait a few ticks, then destroy
	Sleep( 0.0001 );
	Sleep( 0.0001 );
	Sleep( 0.0001 );
	Destroy();	
}


//*     *     *     *     *     *     *     *     *     *     *     *     *     *     *     *     *     *     *
// *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   *
//  * *	  * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * *   * * 
//   *	   *     *     *     *     *     *     *     *     *     *     *     *     *     *     *     *     *  

//***********************************************************************************************************
//command is like "MoveTo Target Arg1 Arg2 ....."
//cue is the cue to send when the action is complete. Save this somewhere for when the command is finished.
//bFastFlag is for the fastforward stuff. Its not hooked up to anything yet tho.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;

	local actor tempActor;
	local string targetName;

	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	if( sActualCommand ~= "Capture" )
	{
		Acceleration = vect(0,0,0);
		Velocity =     vect(0,0,0);

		if( HasAnim( IdleAnimName ) )
			LoopAnim( IdleAnimName, 1.0, 0.75 );

		return true;
	}
	else
	if( sActualCommand ~= "Release" )
	{
		if( ShouldPlayIdleOnRelease() )
			LoopAnim( IdleAnimName, 1.0, 0.75 );

		//playerharry.ClientMessage("***** "$name$" release (destroy controllers)");

		DestroyControllers();
		return true;
	}
	else
	if( sActualCommand ~= "Say" )
	{
		return CutCommand_Say(command,cue,bFastFlag);
	}
	else
	if( sActualCommand ~= "FlyTo" )
	{
		return CutCommand_FlyTo( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "FlyToUnLocked"  ||  sActualCommand ~= "FrayUnlocked"  ||  sActualCommand ~= "Funckled"  ||  sActualCommand ~= "Fuct")
	{
		_FlyToController.DisableController();
	}
	else
	if( sActualCommand ~= "MatchRot" )
	{
		return CutCommand_MatchRot( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "LeadActor" )
	{
		return CutCommand_LeadActor( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "EaseBetweenLinearness" )
	{
		fEaseBetweenLinearness = float( ParseDelimitedString( command, " ", 2, false ) );
		return true;
	}
	else
	if( sActualCommand ~= "SetOnPatrolPointPath" )
	{
		return CutCommand_SetOnPatrolPointPath( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "GotoState" )
	{
		GotoState( name(ParseDelimitedString( command, " ", 2, false )) );
		CutCue( cue );
		return true;
	}
	else
	if( sActualCommand ~= "VisibleInSoftware" )
	{
		bRelevantInSoftwareRenderer=true;
		CutCue( cue );
		return true;
	}
	else
	if( sActualCommand ~= "InVisibleInSoftware" )
	{

		bRelevantInSoftwareRenderer=false;
		CutCue( cue );
		return true;
	}
	else
	if( sActualCommand ~= "FadeTo" )
	{
		return CutCommand_FadeTo( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "CastSpell" )
	{
		return CutCommand_CastSpell( command, cue, bFastFlag );
	}

	return  super.CutCommand(command, cue, bFastFlag);
}

//*************************************************************************************************
function bool CutCommand_CastSpell(string command, optional string cue, optional bool bFastFlag)
{
	local baseSpell	CastedSpell;
	local string	sString, SpellName, SpellTarget;
	local actor		aTarget;
	local bool		bFoundTarget;
	local int		i;
	
	// all other camera keywords are unique to camera settings
	for( i = 1; i < 4; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		if( Left(sString, 12) ~= "SpellTarget=" )
			SpellTarget = Mid(sString,12);
		else
		if( Left(sString, 10) ~= "SpellName=" )
			SpellName = Mid(sString,10);
		else
		if( sString == "" )
			break;
	}//end for
	
	// parameter checking
	if( SpellTarget == "" )
	{
		CutErrorString = "CastSpell Command is Missing SpellTarget paramater";
		CutNotifyActor.CutCue( cue );
		return false;
	}
	
	// see if we can find the targetActor
	foreach AllActors( class'Actor', aTarget ) 
	{
		if( aTarget.Name == name(SpellTarget) || aTarget.CutName == SpellTarget )
		{
			bFoundTarget = true;
			break;
		}
	}
	
	// error checking
	if( !bFoundTarget )
	{
		CutErrorString = "Could not find Actor in level with name: " $SpellTarget;
		CutNotifyActor.CutCue( cue );
		return false;
	}
	
	CastedSpell = SpawnSpell( baseWand(playerHarry.weapon).GetClassFromSpellName(SpellName), aTarget );
	CastedSpell.SetLocation( Location + vec(0,0,EyeHeight ) );
	
	if( CastedSpell == None )
	{
		CutErrorString = "Could not Cast spell on target:" $aTarget $" using spellName:" $SpellName;
		CutNotifyActor.CutCue( cue );
		return false;
	}
	
	// If this hpawn has a wand then place the spell at the endPoint
	if( baseWand(weapon) != none )
	{
		CastedSpell.SetLocation( baseWand(weapon).GetWandEndPoint() );
	}

	CutNotifyActor.CutCue( cue );
	return true;
}

//*************************************************************************************************************************
// Template ObjectName
//                      !!!!!!!!!!!!!!!!!!!!! LEAVE THIS TEMPLATE RIGHT AFTER THE CUTCOMMAND FUNCTION !!!!!!!!!!!!!!!!!!

function bool CutCommand_Template(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local string       sString;
	local int          i;

	i = 2;

	//This is if you need to look for an actor
	if( false )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		ForEach AllActors( class'actor', a )
			if( a.CutName ~= sString )
				break;
		if( a == none )
		{
			CutErrorString = "No Actor with CutName '" $ sString $ "'";
			CutCue( cue );
			return false;
		}
		i++;  //skip this parameter...
	}

	if( bFastFlag )
	{
		//Do what needs to get done NOW, then call the cut cue...

		CutCue( cue );
		return true;
	}

	for( i=i; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		//else
		//if( Left(sString, len("time=")) ~= "time=" )
		//{
		//	t = float( Mid(sString, len("time=")) );
		//	bTimeSupplied = true;
		//}
	}

	//Do your stuff...

	//And set the cut notify cue
	sCutNotifyCue = cue;

	return true;
}

//*************************************************************************************************************************
// LeadActor ActorCutName StartPatrolPointCutName DestPatrolPointCutName anim=WaveAnim speech=speechIndex
//  speechIndex is index into LeadSpeechSnd[] array.
function bool CutCommand_LeadActor(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local actor        ActorToLead;
	local name         StartPatrolPoint, DestPatrolPoint;
	local string       sString;
	local int          i;
	local name         anim;
	local string       speech;

	i = 2;

	//ActorCutName
	sString = ParseDelimitedString( command, " ", i, false );
	ForEach AllActors( class'actor', ActorToLead )
		if( ActorToLead.CutName ~= sString )
			break;
	if( ActorToLead == none )
	{
		CutErrorString = "No Actor with CutName '" $ sString $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}
	i++;  //skip this parameter...


	//StartPatrolPoint
	sString = ParseDelimitedString( command, " ", i, false );
	ForEach AllActors( class'actor', a )
		if( a.CutName ~= sString )
			break;
	if( a == none )
	{
		CutErrorString = "No Actor with CutName '" $ sString $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}
	StartPatrolPoint = a.name;
	i++;  //skip this parameter...

	//DestPatrolPoint
	sString = ParseDelimitedString( command, " ", i, false );
	ForEach AllActors( class'actor', a )
		if( a.CutName ~= sString )
			break;
	if( a == none )
	{
		CutErrorString = "No Actor with CutName '" $ sString $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}
	DestPatrolPoint = a.name;
	i++;  //skip this parameter...


	for( i=i; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
		if( Left(sString, 5) ~= "anim=" )
			anim = name( Mid(sString, 5) );
		else
		if( Left(sString, 8) ~= "BumpSet=" )
			speech = Mid(sString, 8);
	}


	//Do your stuff...
	LeadActor( ActorToLead, StartPatrolPoint, DestPatrolPoint, anim, speech );

	//And set the cut notify cue
	sCutNotifyCue = cue;

	return true;
}

//*************************************************************************************************
function bool CutCommand_FadeTo(string command, optional string cue, optional bool bFastFlag)
{
	local FadeActorController control;
	local TimedCue	tcue;
	local string	sString;
	local float		fTime, fOpacity;
	local int		i;
	
	// set defaults
	fTime	   = 1.0f;
	fOpacity   = 0.5f;

	// all other camera keywords are unique to camera settings
	for( i = 1; i < 4; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		if( Left(sString, 10) ~= "Opacity=" )
			fOpacity = float(Mid(sString,8));
		else
		if( Left(sString, 5) ~= "Time=" )
			fTime = float(Mid(sString,5));
		else
		if( sString == "" )
			break;
	}//end for

	if( bFastFlag )
	{
		CutCue( cue );
	}
	else // we want a cut cue
	{
		//create a TimedCue to cue object after sndLen seconds.
		tcue = spawn(class 'TimedCue');
		tcue.CutNotifyActor=CutNotifyActor;	//Tell me when done. This is auto passed back to the CutNotifyActor if any.
											//Or it can be used by the talk to find out when the talk is finished.
		tcue.SetupTimer(fTime+0.5,cue);		//little extra time for slop	
	}

	//Camera fade...
	control = spawn(class'FadeActorController');
	control.Init(self, fOpacity, fTime );

	return true;
}

//*************************************************************************************************
function bool CutCommand_MatchRot(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local string       sString;
	local int          i;
	local Rotator      r;

	i = 2;

	//This is if you need to look for an actor
	sString = ParseDelimitedString( command, " ", 2, false );

	ForEach AllActors( class'actor', a )
		if( a.CutName ~= sString )
			break;

	if( a == none )
	{
		CutErrorString = "No Actor with ObjectName '" $ sString $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}

	i++;  //skip the 'actor' parameter...


	for( i=i; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		//else
		//if( Left(sString, 5) ~= "time=" )
		//{
		//	t = float( Mid(sString, 5) );
		//	bTimeSupplied = true;
		//}
	}

	//Do your stuff...
	if( a.IsA('InterpolationPoint') )
		r = rotator(InterpolationPoint(a).StartControlPoint);
	else
		r = a.Rotation;

	//Do what needs to get done NOW, then call the cut cue...
	DesiredRotation = r;
	if( Physics == PHYS_None )
		SetPhysics( PHYS_Rotating );
	//FT: uncommented these.  If you're setting DesiredRotation, you better set these.
	bRotateToDesired = true;
	bFixedRotationDir = false;

	if( bFastFlag )
		SetRotation( r );

	CutNotifyActor.CutCue( cue );
	return true;
}

//FlyTo  ActorName  Snap/Linear/EaseFrom/EaseTo/EaseBetween  Fixed/Relative  x=n y=n z=n  Time=t/Speed=s
// Fixed:x,y,z offset from char.  Relative:x,y,z offset transformed by char's yaw, then added.
function bool CutCommand_FlyTo(string command, optional string cue, optional bool bFastFlag)
{
	local string       sCutName;
	local actor        a;
	local string       sString;
	local float        f;
	local enumMoveType MoveType;
	local bool         bFixedToChar;
	local int          i;
	local vector       v, v2;
	local bool         bTimeSupplied;
	local bool         bCalcTimeFromSpeed;
	local float        Speed;
	local float        t;
	local bool         bStayLockedToActor;
	local bool         bRelativeJustForInit;

	bFixedToChar = true;          //default to fixed,
	bRelativeJustForInit = true;  // with relative initial offsets.

	MoveType = MOVE_TYPE_EASE_FROM_AND_TO;

	sCutName = ParseDelimitedString( command, " ", 2, false );

	//playerHarry.ClientMessage("FlyTo: "$command);

	//First off, stop the conroller, if there is one...
	if( _FlyToController != none )
		_FlyToController.DisableController();

	ForEach AllActors( class'actor', a )
		if( a.CutName ~= sCutName )
			break;

	if( a == none )
	{
		//PlayerHarry.ClientMessage("**** ERROR: FlyTo: No Actor with CutName '" $ sCutName $ "'";
		CutErrorString = "FlyTo: No Actor with CutName '" $ sCutName $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}

	if( bFastFlag )
	{
		SetLocation( a.Location );
		CutNotifyActor.CutCue( cue );
		return true;
	}

	for( i = 3; i < 15; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;

		if( sString ~= "Snap" )
			MoveType = MOVE_SNAP;
		else
		if( sString ~= "Linear"  ||  sString ~= "l")
			MoveType = MOVE_TYPE_LINEAR;
		else
		if( sString ~= "EaseFrom"  ||  sString ~= "ef")
			MoveType = MOVE_TYPE_EASE_FROM;
		else
		if( sString ~= "EaseTo"  ||  sString ~= "et")
			MoveType = MOVE_TYPE_EASE_TO;
		else
		if( sString ~= "EaseBetween" )
			MoveType = MOVE_TYPE_EASE_FROM_AND_TO;
		else
		if( sString ~= "Relative"   ||  sString ~= "Rel")
			bFixedToChar = false;  //we want truly relative
		else
		if( sString ~= "Fixed" )
			bRelativeJustForInit = false;  //keep fixed, just dont use relative for initial offset
		else
		if( Left(sString, 2) ~= "x=" )
			v.x = float( Mid(sString,2) );
		else
		if( Left(sString, 2) ~= "y=" )
			v.y = float( Mid(sString,2) );
		else
		if( Left(sString, 2) ~= "z=" )
			v.z = float( Mid(sString,2) );
		else
		if( Left(sString, 5) ~= "time=" )
		{
			t = float( Mid(sString, 5) );
			bTimeSupplied = true;
		}
		else
		if( Left(sString, 6) ~= "speed=" )
		{
			Speed = float( Mid(sString, 6) );
			bCalcTimeFromSpeed = true;
			bTimeSupplied = true;
		}
		else
		if( sString ~= "StayLocked"  ||  sString ~= "sl" )
			bStayLockedToActor = true;
		else
			playerHarry.ClientMessage( "***** ERROR:"$self$":FlyTo option '"$sString$"' not recognised.  Ignoring.");
	}

	//Find a default time, if none supplied
	if( !bTimeSupplied  ||  t == 0 )
		//Nah, if no time supplied, do a snap
		MoveType = MOVE_SNAP;


	//v = a.Location + v2;


	if( bRelativeJustForInit   &&   v != vect(0,0,0) )
		v = v >> a.Rotation;

	//Look for special snap case.   We want to set the location now, even if we stay locked to the actor.
	if( MoveType == MOVE_SNAP )
	{
		SetLocation2( a.Location + v );

		//We also want to send the que now, regardless of whether we're locked to the actor.
		if( true )//!bStayLockedToActor )
		{
   			CutNotifyActor.CutCue( cue );
			//return true;
		}
	}

	if( bCalcTimeFromSpeed )
		t = VSize( a.Location - Location ) / Speed;

	sCutNotifyCue = cue;

	//DoFlyTo( v2, MoveType, t );
	DoFlyTo_Actor( a, v, MoveType, t, bFixedToChar, bStayLockedToActor );

	return true;
}

//*************************************************************************************************************************
// SetOnPatrolPointPath CutName
function bool CutCommand_SetOnPatrolPointPath(string command, optional string cue, optional bool bFastFlag)
{
	local PatrolPoint  a;
	local string       sString;
	local int          i;

	sString = ParseDelimitedString( command, " ", 2, false );

	ForEach AllActors( class'PatrolPoint', a )
		if( a.CutName ~= sString )
			break;

	if( a == none )
	{
		CutErrorString = " SetOnPatrolPointPath: No Actor with ObjectName '" $ sString $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}

	for( i = 3; i < 15; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
	}

	firstPatrolPointObjectName = a.name;
	//DestPatrolPoint = none;
	//bFollowPatrolPoints = true;
	ePatrolType = PATROLTYPE_PATROL_POINTS;
	bGoBackToLastNavPoint = false;
	navP = none;
	tempNavP = none;
	LastNavP = none;
	GotoState('patrol');

	return true;
}

//*************************************************************************************************************************
function float PlayDialog(
	string              dlgId,
	optional ESoundSlot Slot,
	optional float		Volume,
	optional bool		bNoOverride,
	optional float		Radius,
	optional float		Pitch,
	optional bool		Disable3D,
	optional bool		Loop
	)
{
	local sound  dlgSound;
	local string dlgString;
	local float  sndLen;
	local int    i;

	if( Slot == SLOT_None )
		Slot = SLOT_Talk;

	dlgString=Localize( "all", dlgId,"HPdialog" );
	dlgSound = Sound( DynamicLoadObject("AllDialog."$dlgId, class'Sound') );
	
	if(dlgSound!=None)
	{
		sndLen=GetSoundDuration(dlgSound);
		if( Volume == 0 )
			Volume = 1000;
		if( Radius == 0 )
			Radius = 700;
		if( Pitch == 0 )
			Pitch = 1;
		PlaySound(dlgSound, Slot, Volume, bNoOverride, Radius, Pitch, Disable3D, Loop);
	}
	else
	{
		//make up a duration if no sound
		sndLen=(Len(dlgString)*0.01)+3.0;
	}

	return sndLen;
}

//*************************************************************************************************************************
function float PlayDialogLoud(
	string              dlgId,
	optional ESoundSlot Slot,
	optional float		Volume,
	optional bool		bNoOverride,
	optional float		Radius,
	optional float		Pitch,
	optional bool		Disable3D,
	optional bool		Loop
	)
{
	return PlayDialog( dlgId, Slot, 1000000, bNoOverride, Radius, Pitch, Disable3D, Loop );
}

//*************************************************************************************************************************
//This function should be in object.uc
function float EaseBetween(float t)
{
	if( t <= 0 )
		return 0;
	else
	if( t < 0.5 )
		return 2.0*t*t;
	else
	if( t < 1.0 )
		return 1.0 - 2.0*(1.0-t)*(1.0-t);
	else
		return 1.0;
}

//*************************************************************************************************************************
//This function should be in object.uc
// 2 is not linear at all, infinite is linear.
function float EaseBetween2(float t, float HowLinear)
{
	local float x;
	local float y;
	local float m;
	local float b;

	if( HowLinear <= 2 )
		return EaseBetween( t );

	x = ( HowLinear - sqrt(HowLinear*HowLinear - 2*HowLinear) ) / ( 2*HowLinear );
	m = 2*HowLinear*x;
	b = -HowLinear*x*x;

	if( t <= 0 )
		return 0;
	else
	if( t < x )
		return HowLinear*t*t;
	else
	if( t < 1.0 - x )
		return m*t + b;
	else
		return 1.0 - HowLinear*(1.0-t)*(1.0-t);
}

//*************************************************************************************************************************
//This function should be in object.uc
//m = 
const EaseFromX =  0.2928932188; //<-- x,y is the exact point at which the parabola turns to a straight line and goes straight up to 1,1
const EaseFromY =  0.171573;    //2*x*x;  //These y,m,and b numbers are interesting...
const EaseFromM =  1.171573;    //(1 - y) / (1 - x);
const EaseFromB = -0.171573;    //(y - x) / (1 - x);
//const OneOverM =   0.8535533;

function float EaseFrom(float t)
{
	if( t <= 0 )
		return 0;
	else
	if( t < EaseFromX )
		return 2.0*t*t;
	else
	if( t < 1.0 )
		return EaseFromM*t + EaseFromB;
	else
		return 1.0;
}

function float EaseTo(float t)
{
	if( t <= 0 )
		return 0;
	else
	if( t < 1-EaseFromX )
		return EaseFromM*t + 0;
	else
	if( t < 1.0 )
		return 1.0 - 2.0*(1.0-t)*(1.0-t);
	else
		return 1.0;
}

function float EaseFunction(float t, enumMoveType EaseType )
{
	t = FClamp(t, 0, 1);

	switch( EaseType )
	{
		case MOVE_TYPE_EASE_FROM:
			return EaseFrom( t );
		case MOVE_TYPE_EASE_TO:
			return EaseTo( t );
		case MOVE_TYPE_EASE_FROM_AND_TO:
			return EaseBetween( t );
	}

	return t;
}

//Returns a vector that is set to Loc, but then the z is set from this HPawn's location.  Or, if you pass
// in ZRef, the z comes from ZRef;
function vector LocationSameZ( vector Loc, optional vector ZRef )
{
	local vector v;
	v = Loc;

	if( ZRef != vect(0,0,0) )
		v.z = ZRef.z;
	else
		v.z = Location.z;

	return v;
}

function PlayerCutCapture()
{
	// If needed this function should be overwritten by the individual pawns. 
	// If overwritten should stop any behavior not wanted in a cut scene. 
}

function PlayerCutRelease()
{
	// If needed this function should be overwritten by the individual pawns. 
	// If overwritten should start normal behavior after a cut scene. 
}

function SetDespawnFlag()
{
	bDespawned = true;
}

//**************************************************************************************
defaultproperties
{
	DrawType=DT_MESH
	Mesh=skSunDialMesh
	Physics=PHYS_NONE
	fThrowVelocity=400
	ShadowScale=1.0

	fEaseBetweenLinearness=2
	bThrownObjectDamage=False
	bGoToClosestPatrolPoint=False
	bPlayRunAnim=True

	fCurrTime=0.0
	bDespawned=false
	bDespawnable=false

	bAccurateThrowing=false

}
