class Snape extends Characters;

var float  fWaitTime;
var float  fCurrTime;
var float  fDuration;

var int    iPointsToDeduct;

function AdjustHousePoints(int points)
{
	local StatusGroup sgHousePoints;
	local int count;

	// nothing to deduct, do nothing
	if(points == 0)
		return;

	count = HousePointsCount();
	if( count + points < 0)
		points = -count;

	sgHousePoints = playerHarry.managerStatus.GetStatusGroup(class'StatusGroupHousePoints');
	sgHousePoints.IncrementCount(class'StatusItemGryffindorPts', points);
}

function int HousePointsCount()
{
	local StatusGroup sgHousePoints;
	local int count;

	sgHousePoints	= playerHarry.managerStatus.GetStatusGroup(class'StatusGroupHousePoints');
	count			= sgHousePoints.GetStatusItem(class'StatusItemGryffindorPts').nCount;
	return count;
}

function float PlaySnapeSoundText()
{
	local float duration;

	switch( Rand(6) )
	{
		case 0:	
			duration = DeliverLocalizedDialog("PC_Snp_SnpDeductHP_03",true,0.0);	// [Mad]What are you doing, Potter?
			iPointsToDeduct = 0;
			break;
		case 1:	
			duration = DeliverLocalizedDialog("PC_Snp_SnpDeductHP_04",true,0.0);	// [Mad]5 Points from Gryffindor!                                 
			iPointsToDeduct = 5;
			break;
		case 2:	
			duration = DeliverLocalizedDialog("PC_Snp_SnpDeductHP_05",true,0.0);	// [Mad]10 Points from Gryffindor!                                
			iPointsToDeduct = 10;
			break;
		case 3:	
			duration = DeliverLocalizedDialog("PC_Snp_SnpDeductHP_06",true,0.0);	// [Mad]15 Points from Gryffindor!                                
			iPointsToDeduct = 15;
			break;
		case 4:	
			duration = DeliverLocalizedDialog("PC_Snp_SnpDeductHP_07",true,0.0);	// [Mad]20 Points from Gryffindor!                                
			iPointsToDeduct = 20;
			break;
		case 5:	
			duration = DeliverLocalizedDialog("PC_Snp_SnpDeductHP_08",true,0.0);	// [Mad]Now go, or I shall deduct even more House Points from you.
			iPointsToDeduct = 0;
			break;

		default:
			duration = 0;
			iPointsToDeduct = 0;
			break;
	}

	return duration;
}

function PreBeginPlay()
{
	Super.PreBeginPlay();
//	AdjustHousePoints(100);
}

function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	// if Snape does not patroling, do nothing
	if( GetStateName() != 'patrol' )
		return;


	// if harry is already captured by a cutscene, do nothing
	if(playerHarry.CutNotifyActor != None)
		return;

	// if Harry does not have points, do nothing
	if(HousePointsCount() <= 0)
		return;

	// looking for Harry every 1 second ( 10 seconds, if just spoke to him ).
	fCurrTime += DeltaTime;
	if(fCurrTime < fWaitTime)
		return;

	fCurrTime = 0.0f;
	fWaitTime = 1.0f;

	if( vsize(location - playerHarry.location) < SightRadius )
	{
		if(LineOfSightTo(PlayerHarry))
		{
			// remember his last location
			vLastLocation = Location;

			gotostate('stateGotoHarry');
		}
		else
		{
		}
	}
}

function Bump(actor other)
{
 	// if he bumps to somebody but Harry, do nothing
	if(other != playerHarry)
		return;

 	// if he was not going to Harry, do nothing
	if( GetStateName() != 'stateGotoHarry' )
		return;

	gotostate('stateSaySomething');
}

state stateSaySomething
{
	begin:

	fWaitTime=10.0f;

	//if harry is captured by a cutscene, do nothing
	if(playerHarry.CutNotifyActor == None)
	{
		// Stop moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);

		// Turn to Harry
		TurnTo(playerHarry.location);

		playerHarry.CutCommand("Capture","");	//this disables harry and enters cutscene mode.
		playerHarry.CutNotifyActor = self;	 	//setup notify for CutCue.

		fDuration = PlaySnapeSoundText();

		switch( Rand(3) )
		{
			case 0:
				LoopAnim('talk_rhand');
				break;

			case 1:
				LoopAnim('talk_lhand');
				break;

			case 2:
				LoopAnim('talk_bothhands');
				break;
		}

		Sleep(fDuration - 1);
		FinishAnim();

		LoopAnim('idle');
		Sleep(1);
		FinishAnim();

		playerHarry.CutCommand("Release","");	//release harry and exit cutscene mode.
		playerHarry.CutNotifyActor = None;	  	//Clear notify actor.

		AdjustHousePoints(-iPointsToDeduct);

	}

	gotostate('patrol');
}

state stateGotoHarry
{
	begin:

	LoopAnim('walk');
	MoveTo( playerHarry.location );
	FinishAnim();

	gotostate('patrol');	//	gotostate('stateSaySomething');
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skProfSnapeMesh'

	AmbientGlow=65

	CollisionRadius=20
	CollisionHeight=48

	fWaitTime=1.000000
	fCurrTime=0.000000

	SightRadius=150

	bLoopPath=true

	IdleAnimName=walk
	RunAnimName=walk
}
