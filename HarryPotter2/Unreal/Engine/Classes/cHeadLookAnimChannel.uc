//====================================================================================
// cHeadLookAnimChannel	-- Animation Channel for control the direction of pawn's head
//====================================================================================
// After creating this channel, you can use these operations:
//	LookAtDirection				-- Point head at a specific physical relative direction
//	LookAtDirectionOfRotator	-- Point head in the relative direction indicated by a rotator
//	LookAtDirectionOfVector		-- Point head in the relative direction indicated by a vector
//	LookAtPosition				-- Point head in the absolute direction of the indicated point in space
//	LookAtActor					-- Point head in the absolute direction of the indicated actor
//	WatchPosition				-- Continually look at a position
//	WatchTarget					-- Continually look at a target, with an optional offset
//	StopLooking					-- Disable explicit head pointing
//====================================================================================
class cHeadLookAnimChannel expands AnimChannel;

var Name	CurrAnim;			// The name of the animation currently playing
var vector	CurrWatchPosition;	// Current position to watch
var actor	CurrWatchTarget;	// Current actor to watch
var vector	CurrWatchOffset;	// Offset from WatchTarget to watch

enum YawDir				// Quantized directions, yaw
{
	Y_L90,		// 0
	Y_L45,		// 1
	Y_Forward,	// 2
	Y_R45,		// 3
	Y_R90		// 4
};

enum PitchDir			// Quantized directions, pitch
{
	P_Up,		// 0
	P_Level,	// 1
	P_Down,		// 2
};

struct AnimInfo					// Info on an anim available for a specific yaw and pitch direction
{
	var name		AnimName;		// The name of the anim
};

struct YawAnimInfo				// Info on all anims available for a specific yaw direction
{
	var AnimInfo	Pitch[3];		// The yaw anim info broken down by pitch
};

struct DirAnimInfo				// Info on all anims available for all directions
{
	var YawAnimInfo	Yaw[5];			// The anim info broken down by yaw
};

var DirAnimInfo		AnimTable;	// Table of info on all available directional animations

//-------------------------------------------------------------------------------------------
// Operational methods
//-------------------------------------------------------------------------------------------

function LookInDirection( YawDir NewYawDir, PitchDir NewPitchDir )
{
	// Look in the specified direction
	local Name		AnimName;

	// Lookup the anim name and play it
	AnimName = AnimTable.Yaw[ NewYawDir ].Pitch[ NewPitchDir ].AnimName;
	if ( AnimName != CurrAnim )
	{
		if ( GetStateName() == 'Idle' )
			GotoState( 'Looking' );
		PlayAnim( AnimName, 1.0, 0.8 );
		CurrAnim = AnimName;
	}
}

function LookInDirectionOfRotator( rotator NewLookRot )
{
	// Find anim nearest to desired direction and play it
	local YawDir	eYawDir;
	local PitchDir	ePitchDir;
	local Name		AnimName;

	// Quantize the yaw direction
	NewLookRot.Yaw = NewLookRot.Yaw & 0x0000FFFF;
	if		( NewLookRot.Yaw > 0xF000 )	eYawDir = Y_Forward;
	else if ( NewLookRot.Yaw > 0xD000 )	eYawDir = Y_L45;
	else if ( NewLookRot.Yaw > 0x8000 )	eYawDir = Y_L90;
	else if ( NewLookRot.Yaw > 0x3000 )	eYawDir = Y_R90;
	else if ( NewLookRot.Yaw > 0x1000 )	eYawDir = Y_R45;
	else								eYawDir = Y_Forward;

	// Quantize the pitch direction
	NewLookRot.Pitch = NewLookRot.Pitch & 0x0000FFFF;
	if		( NewLookRot.Pitch > 0xF000 )	ePitchDir = P_Level;
	else if ( NewLookRot.Pitch > 0x8000 )	ePitchDir = P_Down;
	else if ( NewLookRot.Pitch > 0x1000 )	ePitchDir = P_Up;
	else									ePitchDir = P_Level;

	// Lookup the anim name and play it
//	Log( Name$" Looking in Direction "$NewLookRot$", Yaw="$eYawDir$", Pitch="$ePitchDir$"." );
	LookInDirection( eYawDir, ePitchDir );
}

function LookInDirectionOfVector( vector NewLookDir )
{
	// Look in the direction indicated by the given relative vector
	LookInDirectionOfRotator( rotator( NewLookDir ) );
}

function LookAtPosition( vector NewLookPosition )
{
	// Look in the direction of the indicated point in space
	LookInDirectionOfVector( (NewLookPosition - Owner.Location) << Owner.Rotation );
}

function LookAtActor( actor NewLookTarget, optional vector Offset )
{
	// Look in the direction of the indicated actor in space, with an optional offset
	LookAtPosition( NewLookTarget.location + Offset );
}

function WatchPosition( vector NewWatchPosition )
{
	// Continually look at the indicated point in space
	CurrWatchPosition = NewWatchPosition;
	GotoState( 'WatchingPosition' );
}

function WatchActor( actor NewWatchTarget, optional vector Offset )
{
	// Continually look at the indicated actor in space, with an optional offset
	CurrWatchTarget = NewWatchTarget;
	CurrWatchOffset = Offset;
	GotoState( 'WatchingTarget' );
}

function StopLooking()
{
	// Disable anim channel
	GotoState( 'Idle' );
	CurrAnim = '';
}

//-------------------------------------------------------------------------------------------
// States
//
// Idle				- Not actively looking at anything
// Looking			- Passively looking in a certain direction
// WatchingPosition	- Actively watching at a specified position
// WatchingTarget	- Actively watching at a specified target
//-------------------------------------------------------------------------------------------

auto state Idle
{
Begin:
//	Log( Owner.Name$"'s Head Look Channel: Entered '"$GetStateName()$"' State" );
	bAnimNotReplaceable = false;
}

state Looking
{
Begin:
//	Log( Owner.Name$"'s Head Look Channel: Entered '"$GetStateName()$"' State" );
	bAnimNotReplaceable = true;

Loop:
	Sleep( 0.1 );
	goto 'Loop';
}

state WatchingPosition extends Looking
{
	event Tick( float DeltaTime )
	{
		// Update the look position to maintain a watch on position
		LookAtPosition( CurrWatchPosition );
		Super.Tick( DeltaTime );
	}
}

state WatchingTarget extends Looking
{
	event Tick( float DeltaTime )
	{
		// Update the look position to maintain a watch on target
//		Log( Owner.Name$" Watching "$CurrWatchTarget$"." );
		LookAtActor( CurrWatchTarget, CurrWatchOffset );
		Super.Tick( DeltaTime );
	}
}


defaultproperties
{
	bAnimNotReplaceable=false

	// Yaw = Y_L90
	AnimTable=(Yaw[0]=(Pitch[0]=(AnimName=Look_UpL90)))
	AnimTable=(Yaw[0]=(Pitch[1]=(AnimName=Look_L90)))
	AnimTable=(Yaw[0]=(Pitch[2]=(AnimName=Look_DownL90)))

	// Yaw = Y_L45
	AnimTable=(Yaw[1]=(Pitch[0]=(AnimName=Look_UpL45)))
	AnimTable=(Yaw[1]=(Pitch[1]=(AnimName=Look_L45)))
	AnimTable=(Yaw[1]=(Pitch[2]=(AnimName=Look_DownL45)))

	// Yaw = Y_Forward
	AnimTable=(Yaw[2]=(Pitch[0]=(AnimName=Look_Up)))
	AnimTable=(Yaw[2]=(Pitch[1]=(AnimName=Look_Forward)))
	AnimTable=(Yaw[2]=(Pitch[2]=(AnimName=Look_Down)))

	// Yaw = Y_R45
	AnimTable=(Yaw[3]=(Pitch[0]=(AnimName=Look_UpR45)))
	AnimTable=(Yaw[3]=(Pitch[1]=(AnimName=Look_R45)))
	AnimTable=(Yaw[3]=(Pitch[2]=(AnimName=Look_DownR45)))

	// Yaw = Y_R90
	AnimTable=(Yaw[4]=(Pitch[0]=(AnimName=Look_UpR90)))
	AnimTable=(Yaw[4]=(Pitch[1]=(AnimName=Look_R90)))
	AnimTable=(Yaw[4]=(Pitch[2]=(AnimName=Look_DownR90)))
}
