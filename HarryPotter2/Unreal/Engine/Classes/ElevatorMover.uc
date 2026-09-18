//=============================================================================
// ElevatorMover.
//=============================================================================
class ElevatorMover extends Mover;

// Allows this mover to go from any key frame to any other key frame,
// depending on the trigger settings.  This produces elevator-like control. M

var int   LastKeyNum;
var int   NextKeyNum;
var int   MoveDirection;
var float MoveTimeInterval;
var bool  bMoveKey;
var()	bool bGoStraight;
var()	bool bUseTriggerMoveTime;

var()	name PuzzleName;

function BeginPlay() 
{
	Super.BeginPlay();
	bMoveKey = true;
}

function MoveKeyframe( int newKeyNum, float newMoveTime )
{
	if( !bMoveKey ) return;

	NextKeyNum = newKeyNum;
	if( NextKeyNum < KeyNum )
	{
		MoveDirection = -1;
		MoveTimeInterval = newMoveTime/(KeyNum-NextKeyNum);
		GotoState('ElevatorTriggerGradual','ChangeFrame');
	}
	
	if( NextKeyNum > KeyNum )
	{
		MoveDirection = 1;
		MoveTimeInterval = newMoveTime/(NextKeyNum-KeyNum);
		GotoState('ElevatorTriggerGradual','ChangeFrame');
	}
}

function DoOpen() 
{
	local float localMoveTime;

	// Open through to the next keyframe.
	//
	bOpening = true;
	bDelaying = false;
	LastKeyNum = KeyNum;

	if(bUseTriggerMoveTime)
		localMoveTime = MoveTimeInterval;
	else
		localMoveTime = MoveTime;

	if(!bGoStraight)
		InterpolateTo (KeyNum+1, localMoveTime);
	else
	{
		if(bKeepRotationDirection)
			InterpolateTo (NextKeyNum, localMoveTime );
		else
			InterpolateTo (NextKeyNum, localMoveTime * (NextKeyNum - KeyNum));
	}

	PlaySound(OpeningSound, SLOT_None, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0 );
	PlaySound(MoveAmbientSound, SLOT_misc, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0, , true);
//	AmbientSound = MoveAmbientSound;
}

function DoClose() 
{
	local float localMoveTime;

	// Close through to the next keyframe.
	//
	bOpening = false;
	bDelaying = false;
	LastKeyNum = KeyNum;

	if(bUseTriggerMoveTime)
		localMoveTime = MoveTimeInterval;
	else
		localMoveTime = MoveTime;

	if(!bGoStraight)
		InterpolateTo (KeyNum-1, localMoveTime);
	else
	{
		if(bKeepRotationDirection)
			InterpolateTo (NextKeyNum, localMoveTime );
		else
			InterpolateTo (NextKeyNum, localMoveTime * (KeyNum - NextKeyNum));
	}

	PlaySound(ClosingSound, SLOT_None, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0);
	PlaySound(MoveAmbientSound, SLOT_misc, MoverVolume/128.0, , MoverRadius, MoverPitch/64.0, , true);
//	AmbientSound = MoveAmbientSound;
}

state() ElevatorTriggerGradual 
{

	function InterpolateEnd(actor Other) 
	{	
	}

	function BeginState()
	{
		bOpening = false;
	}

ChangeFrame:
	bMoveKey = false;

	// Move the mover
	//
	if( MoveDirection > 0	){
		DoOpen();
		FinishInterpolation();
		FinishedClosing();
		StopSound(MoveAmbientSound, SLOT_Misc);
	}
	else {
		DoClose();
		FinishInterpolation();
		FinishedOpening();
		StopSound(MoveAmbientSound, SLOT_Misc);
	}

	// Check if there are more frames to go
	//
	if( KeyNum != NextKeyNum )
	{
		GotoState('ElevatorTriggerGradual','ChangeFrame');
	}

	bMoveKey = true;
	Stop;
}

defaultproperties
{
	bGoStraight=false
	bUseTriggerMoveTime=false
	PuzzleName=none
}
