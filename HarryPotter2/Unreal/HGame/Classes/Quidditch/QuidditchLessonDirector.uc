//=============================================================================
// QuidditchLessonDirector -- Main game logic for the Quidditch Lesson
//=============================================================================
class QuidditchLessonDirector extends QuidditchDirector;

var OliverWood		Wood;		// Quidditch lesson instructor

struct LessonEvents				// Event names to trigger at end of quidditch
{
	var() name	Caught;			// Harry just caught the snitch
	var() name	End;			// Director is done running lesson (happens after above event or alone)
};

var(Director) int			SnitchLessonTime;	// How many seconds to spend in snitch lesson; zero means forever
var(Director) LessonEvents	SnitchLessonEvents;	// Event names to trigger for each published event during snitch lesson

var(Director) int			MockGameTime;		// How many seconds to spend in mock game; zero means forever
												// Note: MatchEvents are used for Mock Game

//-------------------------------------------------------------------------------------------
// PreBeginPlay(), PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();
}

function PostBeginPlay()
{
	// Initialize
	Super.PostBeginPlay();

	// Find all additional actors that are subjects to this lesson
	foreach AllActors( class'OliverWood', Wood )
		break;

	// Set Harry so he doesn't get damaged
	Harry.SetInvincible( true );

	// Level starts off with a cutscene
	InitialState='GameCutScene';
}

function OnPlayerTravelPostAccept()
{
	// Called when player gets the TravelPostAccept event.  This is the moment
	// when the player's traveling items become valid.  The match settings can
	// now be collected.

	Super(Director).OnPlayerTravelPostAccept();

	// In lesson, gryffindors play against each other
	Opponent = HA_Gryffindor;

	// Inform all dependant actors what houses are playing
	SetHouses();
}


//-------------------------------------------------------------------------------------------
// States
//
// GameCutScene			- Playing a cut-scene, waiting for a CutCommand to start a lesson
// GameSnitchLesson		- Running the Snitch Catching lesson
// GameMockGame			- Running the mock quidditch match with Gryffindor playing itself
//-------------------------------------------------------------------------------------------

state GameCutScene
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
		Log( Name$" Entered "$GetStateName()$" State" );

		// Put camera into mode to follow Harry
		Harry.Cam.SetCameraMode( CM_Quidditch );
	}

	function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
	{
		local string	sActualCommand;


		sActualCommand = ParseDelimitedString( Command, " ", 1, false );

		if( sActualCommand ~= "StartSnitchLesson" )
		{
			GotoState( 'GameSnitchLesson' );
			CutCue( Cue );
			return true;
		}
		else if( sActualCommand ~= "StartMockGame" )
		{
			GotoState( 'GameMockGame' );
			CutCue( Cue );
			return true;
		}
		else
			return Super.CutCommand( Command, Cue, bFastFlag );
	}
}

state GameSnitchLesson extends GamePlay
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
		Log( Name$" Entered "$GetStateName()$" State" );

		// Put camera into mode to follow Harry
		Harry.Cam.SetCameraMode( CM_Quidditch );

		SetTimer( SnitchLessonTime, false );
	}

	function OnActionKeyPressed()
	{
		local float		fProximity;
		local Bludger	Bludger;

		// Called when the player's "Action" key/button is pressed.

		Super(Director).OnActionKeyPressed();

		// Catch snitch if harry is reaching for it; goto Won state
		if ( bCanReachForSnitch )
		{
			fProximity = VSize( Harry.Location - Snitch.Location );	// Ignore tracking offset

			if ( fProximity < 75 )
			{
				// Caught the Snitch!  Put snitch in harry's hand
				if ( PlayMechanic == PM_ProximityWithHoops )
					Snitch.HoopTrail.GotoState( 'TrailOff' );
				Snitch.StopFlyingOnPath();

				bCanReachForSnitch = false;
				Harry.SetReaching( false );

				Harry.CatchTarget( Snitch, 'IP_HarryWin_Loop' );
				Snitch.Halo.bHidden = true;

	//			QuidHud(Harry.myHUD).DestroyPopup();	/***/

				// Make bludgers stop seeking Harry
				foreach AllActors( class'Bludger', Bludger )
					Bludger.SeekTarget( None );

				// Lesson over; announce catch event and trigger next cutscene
				if ( SnitchLessonEvents.Caught != '' )
					TriggerEvent( SnitchLessonEvents.Caught, self, None );

				TriggerEventDelayed( 5.0, SnitchLessonEvents.End, 'GameCutScene' );
			}
		}
	}

	function Timer()
	{
		// Lesson time over; announce end-of-lesson event (triggers next cutscene)
		if ( SnitchLessonEvents.End != '' )
			TriggerEvent( SnitchLessonEvents.End, self, None );
		GotoState( 'GameCutScene' );
	}
}


state GameMockGame extends GamePlay
{
	function BeginState()
	{
		local QuidditchPlayer	OtherPlayer;

		Super.BeginState();

		// Reset snitch tracking
		bSnitchVisible = false;
		bSeekerJoinedPursuit = false;
		bHarryJoinedPursuit = false;
		bHarryReaching = false;
		Snitch.SwitchPaths();

		// Set players in motion
		foreach AllActors( class'QuidditchPlayer', OtherPlayer )
			OtherPlayer.Trigger( self, None );

		// Make seeker look for snitch
		if ( Seeker != None )
			Seeker.SetLookForTarget( Snitch );

		SetTimer( MockGameTime, false );
	}

	function OnTriggerEvent( Actor Other, Pawn EventInstigator )
	{
		// Something triggered a 'Director' event.
		local Bludger	Bludger;

		// If other is opponent seeker, then seeker has just caught the snitch
		if ( Other == Seeker )
		{
			// If other is opponent seeker, then seeker has just caught the snitch

			// Allow seeker to take snitch
			if ( PlayMechanic == PM_ProximityWithHoops )
				Snitch.HoopTrail.GotoState( 'TrailOff' );
			Snitch.StopFlyingOnPath();
			Snitch.Halo.bHidden = true;

			// Make Harry stop trying to do anything
			bCanReachForSnitch = false;
			Harry.SetReaching( false );
			Harry.SetKickTargetClass( '' );

			// Make bludgers stop seeking Harry
			foreach AllActors( class'Bludger', Bludger )
				Bludger.SeekTarget( None );

			// Lesson over; announce lost event and trigger next cutscene
			if ( MatchEvents.Lost != '' )
				TriggerEvent( MatchEvents.Lost, self, None );

			TriggerEventDelayed( 5.0, MatchEvents.End, 'GameCutScene' );
		}
		else
		{
			// Unexpected trigger event
			Super.Trigger( Other, EventInstigator );
		}
	}

	function OnActionKeyPressed()
	{
		local float		fProximity;
		local Bludger	Bludger;

		// Called when the player's "Action" key/button is pressed.

		Super(Director).OnActionKeyPressed();

		// Catch snitch if harry is reaching for it; goto Won state
		if ( bCanReachForSnitch )
		{
			fProximity = VSize( Harry.Location - Snitch.Location );	// Ignore tracking offset

			if ( fProximity < 75 )
			{
				// Caught the Snitch!  Put snitch in harry's hand
				if ( PlayMechanic == PM_ProximityWithHoops )
					Snitch.HoopTrail.GotoState( 'TrailOff' );
				Snitch.StopFlyingOnPath();

				bCanReachForSnitch = false;
				Harry.SetReaching( false );

				Harry.CatchTarget( Snitch, 'IP_HarryWin_Loop' );
				Snitch.Halo.bHidden = true;

	//			QuidHud(Harry.myHUD).DestroyPopup();	/***/

				// Tell seeker to stop looking for the Snitch
				if ( Seeker != None )
					Seeker.SetLookForTarget( None );

				// Make bludgers stop seeking Harry
				foreach AllActors( class'Bludger', Bludger )
					Bludger.SeekTarget( None );

				// Lesson over; announce catch event and then trigger next cutscene
				if ( MatchEvents.Won != '' )
					TriggerEvent( MatchEvents.Won, self, None );

				TriggerEventDelayed( 5.0, MatchEvents.End, 'GameCutScene' );
			}
		}
	}

	function Timer()
	{
		// Lesson time over; announce end-of-lesson event (triggers next cutscene)
		if ( MatchEvents.End != '' )
			TriggerEvent( MatchEvents.End, self, None );
		GotoState( 'GameCutScene' );
	}
}


defaultproperties
{
	Tag=Director
	bNeedsCommentator=false
	InitialState=GameCutScene

	SnitchLessonTime=60.0		// How many seconds to spend in snitch lesson
	MockGameTime=60.0			// How many seconds to spend in mock game
}
