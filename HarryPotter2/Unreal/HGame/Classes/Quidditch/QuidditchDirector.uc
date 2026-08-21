//=============================================================================
// QuidditchDirector -- Keeper of the rules of the mini-game; main game logic
//=============================================================================
class QuidditchDirector extends Director;

var BroomHarry				Harry;
var Seeker					Seeker;			// Opponent team's seeker
var Snitch					Snitch;
var QuidditchCameraTarget	CameraTarget;	// Point where camera should look while trailing snitch
var QuidditchBar			ProgressBar;	// HUD item for showing snitch progress
var EnemyHealthManager		SeekerHealthBar;// HUD item for showing enemy seeker health
var QuidScoreManager		ScoreBoard;		// HUD item for showing current game score
var StatusGroupHousepoints	HousePoints;	// HUD manager for house points
var QuidditchCommentator	Commentator;	// Source of spoken game commentary

enum QuidPlayMechanic
{
	PM_Hoops,				// Uses hoop trail to judge how well Harry is tracking snitch
	PM_Proximity,			// Uses snitch proximity to judge how well Harry is tracking snitch; no hoop trail
	PM_ProximityWithHoops	// Uses snitch proximity to judge how well Harry is tracking snitch, but hoop trail is visible
};

enum HouseAffiliation	// Must be in same order as in QuidditchCrowd.uc and QuidditchCommentator
{
	HA_Gryffindor,
	HA_Ravenclaw,
	HA_Hufflepuff,
	HA_Slytherin,
};

enum TeamAffiliation	// What team a crowd roots for
{
	TA_Gryffindor,
	TA_Opponent,
	TA_Neutral,

	TA_NumAffiliations
};

struct QuidEvents			// Event names to trigger at end of quidditch
{
	var() name	Won;				// Gryffindor just caught the snitch
	var() name	Lost;				// Opponent just caught the snitch
	var() name	Died;				// Harry just died (forfeit)
	var() name	End;				// Director is done running game (happens after all above events)
};

struct QuidEvents_Final		// Event names to trigger during final match of quidditch
{
	var() name	GoingIntoTrench;	// Start of transition to trench run
	var() name	Won;				// Gryffindor just caught the snitch
	var() name	Lost;				// Opponent just caught the snitch
	var() name	Died;				// Harry just died (forfeit)
	var() name	End;				// Director is done running game (happens after all above events)
};

var(Director) float	fTimeBeforeGoingIntoTrench;	// How long the final game runs before Harry and Draco go into the trench (seconds)
var(Director) bool	bForceFinalMatch;			// Force Quidditch to always play in final match mode (for Debugging)

struct RandomSeed		// To keep game from playing the same way twice
{
	var float SeedA;
	var float SeedB;
	var float SeedC;
	var float SeedD;
};

var(/*Director*/) QuidPlayMechanic	PlayMechanic;

var(Director) HouseAffiliation	Opponent;			// Which house is the opponent team affiliated with

var(Director) QuidEvents		MatchEvents;		// Event names to trigger for each published event in regular quidditch
var(Director) QuidEvents_Final	MatchEvents_Final;	// Event names to trigger for each published event in final match of quidditch

var string			EndCue;							// Cue to give when match has reached the end (game over)

var(/*Director*/) float	fSnitchTrackingOffset;		// How far behind the snitch is the center of the tracking sphere

var(/*Director*/) float	fSnitchMaxGainRadiusAt0;	// How close to snitch Harry needs to be for fastest progress gain
var(/*Director*/) float	fSnitchNeutralRadiusAt0;	// How far from snitch Harry needs to be to not gain or lose progress
var(/*Director*/) float	fSnitchNeutralRadiusAt100;	// How far from snitch Harry needs to be to not gain or lose progress
var(/*Director*/) float	fSnitchMaxLossRadiusAt0;	// How far from snitch Harry needs to be for fastest progress loss

var(/*Director*/) float	fSnitchMaxGainRate;			// How fast Harry gains progress when closest to snitch (percent/sec)
var(/*Director*/) float	fSnitchMaxLossRate;			// How fast Harry loses progress when farthest from snitch (percent/sec)

var(/*Director*/) float	fSnitchMaxCatchTime;		// How long can Harry try to catch the snitch during the catch phase (seconds)
var(/*Director*/) int	SnitchMaxCatchTries;		// How many times can Harry try to catch the snitch during the catch phase

var(/*Director*/) int	HoopsToHit;					// How many hoops have to be hit in a row to catch snitch

const				SnitchTrackDistDecay = 5.0;	// How fast to close gap between Harry and snitch (units per second)
const				SnitchTrackDistMin = 200.0;	// Closest Harry can track the snitch from
const				SnitchTrackDistMax = 300.0;	// Farthest back Harry can track the snitch from
var float			fSnitchTrackDist;			// How far behind the snitch should Harry and seeker track it
var float			CameraTrailDist;			// How far behind the seekers should the camera trail the snitch
var float			fPenaltyGraceExpiration;	// When does penalty grace period expire

var float			fProgressPercent;			// Snitch tracking progress on a scale of 0.0 to 100.0
var int				CatchTriesLeft;				// How many tries left before loosing the snitch

const				NUM_PROGRESS_SOUNDS = 15;
var Sound			ProgressSounds[15];			// All the different tracking progress sounds

var QuidditchCrowd	Crowds[3];					// Chains of spectator crowds, indexed by TeamAffiliation
var float			fTimeToCheer;				// When next to have crowd cheer
var RandomSeed		RandSeed;					// Keep game from playing the same way twice

var bool			bNeedsCommentator;			// Whether this mini-game must have a commentator
var bool			bHousesSet;					// Whether all house-dependant setup has been done yet

var bool			bSnitchVisible;				// Whether Snitch has become visible
var bool			bSeekerJoinedPursuit;		// Whether other seeker has begun chasing snitch yet
var bool			bHarryJoinedPursuit;		// Whether Harry has begun chasing snitch yet
var bool			bHarryReaching;				// Whether Harry is about to grab for the snitch
var bool			bCanReachForSnitch;			// Whether Harry can reach for the snitch

var bool			bGryffWon;					// Whether game ended with Harry catching snitch
var bool			bHarryDied;					// Whether game ended with Harry's death

var int				GryffScore;					// Gryffindor's running score
var int				OpponentScore;				// Opposing team's running score
var bool			bWonCup;					// Whether Gryffindor won the Quidditch Cup

var bool			bFirstTimeThisMatch;		// Whether we're playing this match for the first time
var bool			bFinalMatch;				// Whether we're playing the final match
var bool			bInTrench;					// Whether seeker's are in the trench of final match

var float			fDelayedEventDelayTime;		// How long to wait before triggering a delayed event
var name			DelayedEventName;			// Which event to trigger after delay
var name			DelayedEventNextState;		// Which state to go to after delayed event is triggered, if any
var string			DelayedEventCue;			// Cue to give just before triggering event, if any


//-------------------------------------------------------------------------------------------
// PreBeginPlay(), PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

	// Find commentator (or create one)
	foreach AllActors( class'QuidditchCommentator', Commentator )
		break;
	if ( bNeedsCommentator && Commentator == None )
		Commentator = Spawn( class'QuidditchCommentator' );

	bHousesSet = false;

	// AE:
	PlayBigCheer();
}

function PostBeginPlay()
{
	// Initialize
	Super.PostBeginPlay();

	// Find actors that are subjects to this game
	foreach AllActors( class'BroomHarry', Harry )
		break;
	foreach AllActors( class'Snitch', Snitch )
		break;
	foreach AllActors( class'Seeker', Seeker )	// Find opponent seeker
	{
		if ( Seeker.Team == TA_Opponent )
			break;
	}

	// Create camera target for snitch trailing
	CameraTarget = Spawn( class'QuidditchCameraTarget', None, 'CameraTarget', Snitch.Location, Snitch.Rotation );
	Log( "Spawned camera target '"$CameraTarget.name$"'" );

	// Create HUD items for snitch tracking progress, score, and enemy health
	ProgressBar = Spawn( class'QuidditchBar' );
	ProgressBar.Show( false );

	SeekerHealthBar = Spawn( class'EnemyHealthManager' );

	ScoreBoard = Spawn( class'QuidScoreManager' );

	// Load the tracking progress sounds	/***/
/*
	ProgressSounds[ 0] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop01';
	ProgressSounds[ 1] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop02';
	ProgressSounds[ 2] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop03';
	ProgressSounds[ 3] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop04';
	ProgressSounds[ 4] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop05';
	ProgressSounds[ 5] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop06';
	ProgressSounds[ 6] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop07';
	ProgressSounds[ 7] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop08';
	ProgressSounds[ 8] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop09';
	ProgressSounds[ 9] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop10';
	ProgressSounds[10] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop11';
	ProgressSounds[11] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop12';
	ProgressSounds[12] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop13';
	ProgressSounds[13] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop14';
	ProgressSounds[14] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop15';
*/

	// Other initialization
	bSnitchVisible		 = false;
	bSeekerJoinedPursuit = false;
	bHarryJoinedPursuit	 = false;
	bHarryReaching		 = false;
	bCanReachForSnitch	 = false;
	bGryffWon			 = false;
	bHarryDied			 = false;
	bInTrench			 = false;

	// Level starts off with a cutscene
	InitialState='GameIntro';
}

function OnPlayerTravelPostAccept()
{
	// Called when player gets the TravelPostAccept event.  This is the moment
	// when the player's traveling items become valid.  The match settings can
	// now be collected.
	local string		MatchOpponent;

	Super.OnPlayerTravelPostAccept();

	// Determine match settings
	bFinalMatch = Harry.curQuidMatchNum >= 5;	// Last of 6
	MatchOpponent = Harry.quidGameResults[ Harry.curQuidMatchNum ].opponent;
	if      ( MatchOpponent ~= "Gryffindor" ) Opponent = HA_Gryffindor;
	else if ( MatchOpponent ~= "Ravenclaw"  ) Opponent = HA_Ravenclaw;
	else if ( MatchOpponent ~= "Hufflepuff" ) Opponent = HA_Hufflepuff;
	else if ( MatchOpponent ~= "Slytherin"  ) Opponent = HA_Slytherin;

	bFirstTimeThisMatch = (    Harry.quidGameResults[ Harry.curQuidMatchNum ].myScore == 0
						    && Harry.quidGameResults[ Harry.curQuidMatchNum ].opponentScore == 0 );

	PlayerHarry.ClientMessage( Name$": Start match: "$Harry.curQuidMatchNum$", '"$MatchOpponent$"'" );
	Log( Name$": Start match: "$Harry.curQuidMatchNum$", '"$MatchOpponent$"'" );

	// *** Debug: force trench run, if requested
	if ( bForceFinalMatch )
	{
		bFinalMatch = true;
		Opponent = HA_Slytherin;
	}

	// Inform all dependant actors what houses are playing
	SetHouses();

	// Get a handle to the house point manager
	HousePoints = StatusGroupHousePoints( Harry.managerStatus.GetStatusGroup(class'StatusGroupHousepoints') );
}

//-------------------------------------------------------------------------------------------
// Operational methods
//-------------------------------------------------------------------------------------------

function bool CutQuestion( string Question )
{
	// A cut-scene is asking a question the Director might know the answer to.
	// If the question is unknown, pass it up to parent director.

	if ( Question ~= "OpponentIsGryffindor")
		return Opponent == HA_Gryffindor;
	else if ( Question ~= "OpponentIsRavenclaw")
		return Opponent == HA_Ravenclaw;
	else if ( Question ~= "OpponentIsHufflepuff")
		return Opponent == HA_Hufflepuff;
	else if ( Question ~= "OpponentIsSlytherin")
		return Opponent == HA_Slytherin;

	else if ( Question ~= "FirstMatch")
		return Harry.curQuidMatchNum == 0;
	else if ( Question ~= "FinalMatch")
		return bFinalMatch;
	else if ( Question ~= "FirstTimePlayedMatch")
		return bFirstTimeThisMatch;
	else if ( Question ~= "NotFirstTimePlayedMatch")
		return !bFirstTimeThisMatch;
	else if ( Question ~= "FirstTimePlayedOpponent")
		return Harry.curQuidMatchNum <= 2;
	else if ( Question ~= "SecondTimePlayedOpponent")
		return Harry.curQuidMatchNum >= 3;

	else if ( Question ~= "GryffindorWon")
		return bGryffWon;
	else if ( Question ~= "GryffindorLost")
		return !bGryffWon;

	else
		return Super.CutQuestion( Question );
}

function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
{
	local string		sActualCommand;
	local string		sAssociation;
	local string		sEventName;
	local string		sEventCue;

	sActualCommand = ParseDelimitedString( Command, " ", 1, false );

	if( sActualCommand ~= "SetEventCue" )
	{
		// CutScene wants to associate a cue with an event;
		// remember the cue to give when event happens

		sAssociation = ParseDelimitedString( Command, " ", 2, false );

		if ( sAssociation == "" )
		{
			CutErrorString = "Missing event=cue association";
			CutCue( Cue );
			return false;
		}
		else
		{
			sEventName = ParseDelimitedString( sAssociation, "=", 1, false );
			sEventCue  = ParseDelimitedString( sAssociation, "=", 2, false );

			if ( sEventName ~= "End" )
			{
				EndCue = sEventCue;
				CutCue( Cue );
				return true;
			}
			else
			{
				CutErrorString = "Unknown event in association '" $ sAssociation $ "'";
				CutCue( Cue );
				return false;
			}
		}
	}
	else
		return Super.CutCommand( Command, Cue, bFastFlag );
}

function SetHouses()
{
	// Informs all dependant actors what houses are playing in the match.
	local QuidditchCrowd	Crowd;
	local TeamAffiliation	eTeam;

	// Only do house setup once
	if ( bHousesSet )
	{
		Log( Name$": *** Warning: Houses already setup for match!" );
		return;
	}

	// Tell commentator who the opposing team is
	if ( Commentator != None )
	{
		switch ( Opponent )
		{
			case HA_Ravenclaw:	Commentator.SetOpponent( HA_Ravenclaw );	break;
			case HA_Hufflepuff:	Commentator.SetOpponent( HA_Hufflepuff );	break;
			case HA_Slytherin:	Commentator.SetOpponent( HA_Slytherin );	break;
			default:
				Log( "QuidditchDirector: Warning: Opponent property not set properly" );
		}
	}

	// Change the appearance of the scoreboard
	switch ( Opponent )
	{
		case HA_Gryffindor:	ScoreBoard.SetOpponent( Opponent_Gryffindor );	break;
		case HA_Ravenclaw:	ScoreBoard.SetOpponent( Opponent_Ravenclaw );	break;
		case HA_Hufflepuff:	ScoreBoard.SetOpponent( Opponent_Hufflepuff );	break;
		case HA_Slytherin:	ScoreBoard.SetOpponent( Opponent_Slytherin );	break;
		default:
			Log( "QuidditchDirector: Warning: Opponent property not set properly" );
	}

	// Change the appearance of all players to match their house affiliation
	SetPlayerHouses();

	// Find all the crowds and link them up into separate chains for each team
	foreach AllActors( class'QuidditchCrowd', Crowd )
	{
		Log( "Found crowd "$Crowd.name$", "$Crowd.Affiliation );

		if ( Crowd.Affiliation == HA_Gryffindor )
			eTeam = TA_Gryffindor;
		else if ( Crowd.Affiliation == Opponent )
			eTeam = TA_Opponent;
		else
			eTeam = TA_Neutral;

		// Add crowd to a singly-linked chain for its team
		Crowd.NextCrowd = Crowds[ eTeam ];
		Crowds[ eTeam ] = Crowd;
	}

	bHousesSet = true;
}

function SetPlayerHouses()
{
	// Establishes which house every quidditch player is affiliated with.  Affects
	// their uniform and possibly their mesh.  All Quidditch players already know
	// whether they're on the opposing team or not; this pass associates a house
	// with each team.  The non-opponent team is always Gryffindor house.

	local QuidditchPlayer	OtherPlayer;

	foreach AllActors( class'QuidditchPlayer', OtherPlayer )
	{
		switch ( Opponent )
		{
			case HA_Gryffindor:	OtherPlayer.SetHouse( HA_Gryffindor );	break;
			case HA_Ravenclaw:	OtherPlayer.SetHouse( HA_Ravenclaw );	break;
			case HA_Hufflepuff:	OtherPlayer.SetHouse( HA_Hufflepuff );	break;
			case HA_Slytherin:	OtherPlayer.SetHouse( HA_Slytherin );	break;
			default:
				Log( "QuidditchDirector: Warning: Opponent property not set" );
		}
	}
}

function SetCameraToFollowSnitch()
{
	// Put camera into quidditch mode so that mouse doesn't rotate camera
	Harry.Cam.SetCameraMode( CM_Quidditch );

	// Set camera to follow directly behind snitch
	Harry.Cam.SetTargetActor( CameraTarget.Name );
	Harry.Cam.SetZOffset( 25.0 );
	Harry.Cam.CamTarget.bRelative = true;
	Harry.Cam.SetDistance( CameraTrailDist );
	Harry.Cam.SetRotTightness( 10.0 );
	Harry.Cam.SetMoveTightness( 10.0 );
	Harry.Cam.SetMoveSpeed( 1200 );
//	Harry.Cam.SetSyncPosWithTarget( true );
//	Harry.Cam.SetSyncRotWithTarget( false );
}

// AE:
function PlayBigCheer()
{
//	PlaySound( sound'HPSounds.Quidditch_sfx.big_cheerstream', SLOT_Talk, , , 10000.0 );		/***/
}

function ComputeScore()
{
	// Computes the score for this match and determines if Gryffindor won the
	// Quidditch League with this match.

	// Compute match closing score
	if ( bGryffWon )
	{
		GryffScore += 150;
		ScoreBoard.SetGryffindorScore( GryffScore );
	}
	else
	{
		OpponentScore += 150;
		if ( !bHarryDied )	// Don't show score update if Harry died; score is for opponent catch he'll never see
			ScoreBoard.SetOpponentScore( OpponentScore );
	}

	// Determine if Gryffindor won the Quidditch cup
	bWonCup = bFinalMatch && bGryffWon;
}

function TriggerEventDelayed( float fDelayTime, name EventName,
							  optional name NextState, optional string Cue )
{
	// Wait the for a specified time, give cue, trigger specified event, then goto specified state

	fDelayedEventDelayTime = fDelayTime;
	DelayedEventName = EventName;
	DelayedEventNextState = NextState;
	DelayedEventCue = Cue;

	GotoState( 'PendingEvent' );
}


//-------------------------------------------------------------------------------------------
// States
//
// GameIntro	- Playing intro cut-scene
// GamePlay		- Interactive; flying Harry to get close to the snitch
// GameCatch	- Interactive; timing Harry to actually catch the snitch
// GameWon		- Harry caught snitch
// GameLosing	- Harry lost all stamina and is dying
// GameLost		- Harry died; game lost
// PendingEvent	- Waiting to trigger a delayed event
//-------------------------------------------------------------------------------------------

state GameIntro
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
		Log( Name$" Entered "$GetStateName()$" State" );

		Harry.FlyOnPath( 'IPGSeeker_Intro' );

//		TriggerEvent( 'Intro', self, None );	// Triggered as soon as possible
	}

	function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
	{
		local string			sActualCommand;
		local QuidditchPlayer	OtherPlayer;


		sActualCommand = ParseDelimitedString( Command, " ", 1, false );

		if( sActualCommand ~= "LaunchPlayers" )
		{
			// Intro CutScene wants all active players to fly onto field; launch them
			foreach AllActors( class'QuidditchPlayer', OtherPlayer )
			{
				if ( OtherPlayer.Team != TA_Neutral )
					OtherPlayer.Trigger( self, None );
			}

			CutCue( Cue );
			return true;
		}
		else if( sActualCommand ~= "StartGame" )
		{
			// Intro CutScene ended; start playing quidditch
			Harry.StopFlyingOnPath();
			Harry.AirSpeed = 10;
			Harry.Deceleration = Harry.AirSpeedNormal - Harry.AirSpeed;
			Harry.SetLookForTarget( Snitch );

			// Make seeker and camera look for snitch
			if ( Seeker != None )
				Seeker.SetLookForTarget( Snitch );
			CameraTarget.SetLocation( Harry.Location );
			CameraTarget.SetLookForTarget( Snitch );

			fSnitchTrackDist = SnitchTrackDistMax;
			fPenaltyGraceExpiration = Level.TimeSeconds;

			fTimeToCheer = Level.TimeSeconds;

			switch ( PlayMechanic )
			{
				case PM_Hoops:
					Snitch.HoopTrail.SetHoopsToHit( HoopsToHit );
					break;

				case PM_Proximity:
				case PM_ProximityWithHoops:
					fProgressPercent = 0.0;
					GryffScore = 0;
					OpponentScore = 0;
					break;
			}

			GotoState( 'GamePlay' );

			CutCue( Cue );
			return true;
		}
		else
			return Super.CutCommand( Command, Cue, bFastFlag );
	}

Begin:
Loop:
	Sleep( 0.1 );
	goto 'Loop';
}

state GamePlay
{
	function BeginState()
	{
		local Bludger	Bludger;
		local int		PercentDone;

		// Start seeking the snitch
		PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
		Log( Name$" Entered "$GetStateName()$" State" );

		// Make bludgers look for Harry
		foreach AllActors( class'Bludger', Bludger )
			Bludger.SeekTarget( Harry );

		// Tell Harry it's okay to kick other players now
		Harry.SetKickTargetClass( 'QuidditchPlayer' );

		// Tell other seeker it's okay to kick harry now
		Seeker.SetKickTargetClass( 'BroomHarry' );

		// Set camera to trail behind seekers trailing snitch
		SetCameraToFollowSnitch();

		switch ( PlayMechanic )
		{
			case PM_Hoops:
				ProgressBar.SetProgress( 100 * (Snitch.HoopTrail.HoopsToHit - Snitch.HoopTrail.HoopsToGo) / Snitch.HoopTrail.HoopsToHit );
				break;

			case PM_Proximity:
			case PM_ProximityWithHoops:
				PercentDone = fProgressPercent;
				ProgressBar.SetProgress( PercentDone );
				break;
		}
		ProgressBar.Show( true );
		SeekerHealthBar.Start( Seeker );
		ScoreBoard.StartQuidScore();

		// If final match, set a timer for when seekers should go into trench
		if ( bFinalMatch )
			SetTimer( fTimeBeforeGoingIntoTrench, false );
	}

	function Tick( float DeltaTime )
	{
		local Vector			SnitchDir;
		local Vector			ProximityTestPoint;
		local float				fProximity;

		local float				fSeekerProximity;

		local float				fTrackingOffset;
		local float				fCompression;
		local float				fMaxGainRadius;
		local float				fNeutralRadius;
		local float				fMaxLossRadius;

		local float				fProgressRate;
		local int				PercentDone;
		local int				LastPercentDone;
		local int				ProgressTier;
		local int				NewStage;

		local TeamAffiliation	eTeam;

		local Vector			HarryHeading;
		local Vector			SnitchFromHarry;
		local bool				bSnitchInFront;


		if ( PlayMechanic == PM_Proximity || PlayMechanic == PM_ProximityWithHoops )
		{
			// Update snitch track distance
			fSnitchTrackDist -= SnitchTrackDistDecay * DeltaTime;
			if ( fSnitchTrackDist < SnitchTrackDistMin )
				fSnitchTrackDist = SnitchTrackDistMin;

			Snitch.SetApparentScale( 1.5 * (SnitchTrackDistMax - fSnitchTrackDist)
									     / (SnitchTrackDistMax - SnitchTrackDistMin) + 0.5 );
			Snitch.SetFlashing( fProgressPercent > 98.0 && !(bFinalMatch && !bInTrench) );	// Can't allow a catch before getting into trench during final match

			Harry.SetTargetTrackDist( SnitchTrackDistMax );
			if ( Seeker != None )
				Seeker.SetTargetTrackDist( SnitchTrackDistMax );
			CameraTarget.SetTargetTrackDist( SnitchTrackDistMax );

//			Harry.SetTargetTrackDist( fSnitchTrackDist );
//			if ( Seeker != None )
//				Seeker.SetTargetTrackDist( fSnitchTrackDist );
//			CameraTarget.SetTargetTrackDist( fSnitchTrackDist );

			// Compute radius compression that is based on progress
			// (radii pull in closer to snitch as progress nears completion)
			fTrackingOffset = (1.0 - (fProgressPercent / 100.0)) * fSnitchTrackingOffset;
			fCompression = 1.0 - (fProgressPercent / 100.0) * ((fSnitchNeutralRadiusAt0 - fSnitchNeutralRadiusAt100) / fSnitchNeutralRadiusAt0);
			fMaxGainRadius = fCompression * fSnitchMaxGainRadiusAt0;
			fNeutralRadius = fCompression * fSnitchNeutralRadiusAt0;
			fMaxLossRadius = fCompression * fSnitchMaxLossRadiusAt0;

			// Compute Harry's proximity to snitch
			SnitchDir = Vector( Harry.Rotation );
			ProximityTestPoint = -fTrackingOffset * SnitchDir + Snitch.Location;
			fProximity = VSize( Harry.Location - ProximityTestPoint );

			// Adjust tracking progress based on proximity: closer than
			// Neutral radius gains progress, farther loses progress.
			if ( Snitch.bHidden )
				fProgressRate = -fSnitchMaxLossRate;
			else if ( fProximity >= fMaxLossRadius )
				fProgressRate = -fSnitchMaxLossRate;
			else if ( fProximity <= fMaxGainRadius )
				fProgressRate =  fSnitchMaxGainRate;
			else if ( fProximity > fNeutralRadius )
				fProgressRate = -fSnitchMaxLossRate * (fProximity - fNeutralRadius)
													/ (fMaxLossRadius - fNeutralRadius);
			else
				fProgressRate =  fSnitchMaxGainRate * (fProximity - fNeutralRadius)
													/ (fMaxGainRadius - fNeutralRadius);

			LastPercentDone = fProgressPercent;
//			fProgressPercent += fProgressRate * DeltaTime;
			fProgressPercent = 100 * (SnitchTrackDistMax - fSnitchTrackDist)
								   / (SnitchTrackDistMax - SnitchTrackDistMin);

			if ( fProgressPercent < 0.0 )
				fProgressPercent = 0.0;
			else if ( fProgressPercent > 100.0 )
				fProgressPercent = 100.0;

			// Update the progress element of the HUD
			PercentDone = fProgressPercent;
			if ( PercentDone < LastPercentDone )
				ProgressBar.SetProgress( PercentDone, true, 1.0 );	// Show as Red
			else
				ProgressBar.SetProgress( PercentDone );
//			PlayerHarry.ClientMessage( "Track Progress: "$PercentDone$"%" );

			// If percent done has increased enough, play a progress sound
			ProgressTier = PercentDone / (100.0/NUM_PROGRESS_SOUNDS);
			if ( ProgressTier > (LastPercentDone / (100.0/NUM_PROGRESS_SOUNDS)) )
			{
//				Snitch.PlaySound( ProgressSounds[ ProgressTier-1 ], SLOT_Interact );	/***/
			}

			// If snitch has a hoop trail, update it's stage
			if ( PlayMechanic == PM_ProximityWithHoops )
			{
				NewStage = (5.0 * fProgressPercent/105.0) + 1;
				if ( Snitch.HoopTrail.CurrentStage != NewStage )
				{
					Snitch.HoopTrail.CurrentStage = NewStage;
				}

				// Suppress hoop trail if too close
				if ( fProximity < 480 )
				{
					if ( Snitch.HoopTrail.IsInState( 'TrailOn' ) )
						Snitch.HoopTrail.GotoState( 'TrailOff' );
				}
				else
				{
					if ( Snitch.HoopTrail.IsInState( 'TrailOff' ) )
						Snitch.HoopTrail.GotoState( 'TrailOn' );
				}
			}

			// Determine if Harry can reach for the snitch
			HarryHeading = Vector( Harry.Rotation );
			SnitchFromHarry = Snitch.Location - Harry.Location;
			bSnitchInFront = (SnitchFromHarry dot HarryHeading) > 0.0;

			fSeekerProximity = VSize( Harry.Location - Seeker.Location );

			if (    !Snitch.bHidden && bSnitchInFront && fProximity < 300
				 && fSeekerProximity > 100 )
			{
				if ( !bCanReachForSnitch )
				{
					bCanReachForSnitch = true;
					Harry.SetReaching( true );
					Harry.SetKickTargetClass( '' );
				}
			}
			else
			{
				if ( bCanReachForSnitch )
				{
					bCanReachForSnitch = false;
					Harry.SetReaching( false );
					Harry.SetKickTargetClass( 'QuidditchPlayer' );
				}
			}
			
			// If Harry has tracked the snitch long enough...
//			if ( fProgressPercent >= 100.0 && Seeker.GetStateName() != 'Pursue' )
//			{
				// Caught the Snitch!  Put snitch in harry's hand
/*
				if ( PlayMechanic == PM_ProximityWithHoops )
					Snitch.HoopTrail.GotoState( 'TrailOff' );
				Snitch.StopFlyingOnPath();
				Harry.CatchTarget( Snitch, 'IPHarry_Win' );
*/

//				Harry.SetLookForTarget( None );
//				Harry.SecondaryAnim = 'Hold';	// This will become the loop animation after the 'Catch' finishes
//				Harry.PlayAnim( 'Catch' );

//				Snitch.StopFlyingOnPath();
//				Snitch.SetLocation( Harry.WeaponLoc );
//				Snitch.SetOwner( Harry );
//				Snitch.SetPhysics( PHYS_Trailer );

				// Tell seeker to stop looking for the Snitch
/*
				if ( Seeker != None )
					Seeker.SetLookForTarget( None );
				CameraTarget.SetLookForTarget( Harry );
*/
//				GotoState( 'GameCatch' );
//			}

			// Comment on Harry's pursuit of the snitch
			if ( !bHarryJoinedPursuit )
			{
				if ( !Snitch.bHidden && fProximity < fSnitchNeutralRadiusAt0 )	// If found snitch
				{
					if ( Commentator == None || Commentator.SayComment( QC_HereComesSeeker, TA_Gryffindor ) )
						bHarryJoinedPursuit = true;
				}
			}
			else
			{
				if ( Snitch.bHidden || fProximity > fSnitchMaxLossRadiusAt0	)	// If lost snitch
				{
					if ( bHarryReaching )
					{
						if ( Commentator == None || Commentator.SayComment( QC_MissedSnitch, , true ) )
						{
							bHarryReaching = false;
							bHarryJoinedPursuit = false;
						}
					}
					else
					{
						if ( Commentator == None || Commentator.SayComment( QC_DontGiveUp ) )
							bHarryJoinedPursuit = false;
					}
				}
				else if ( fProximity < fSnitchNeutralRadiusAt0 )	// If closing in
				{
					fProximity = VSize( Harry.Location - Snitch.Location );	// Ignore tracking offset

					if ( !bHarryReaching )
					{
						if ( fProximity < 75.0 || fProgressPercent > 95.0 )		// If close enough to reach
						{
							if ( Commentator != None )
								Commentator.SayComment( QC_ReachingSnitch, , true );
							bHarryReaching = true;
						}
					}
					else if ( !bSnitchInFront && (fProximity > 90.0 && fProgressPercent < 92.0) )	// If missed
					{
						Harry.PlayAnim( 'Miss' );
						if ( Commentator == None || Commentator.SayComment( QC_MissedSnitch, , true ) )
						{
							bHarryReaching = false;
						}
					}

					if ( Commentator != None && fProgressPercent < 95.0 )
					{
						Commentator.SayComment( QC_ClosingOnSnitch );

/*						switch ( Rand(2) )
						{
							case 1: Commentator.SayComment( QC_ClosingOnSnitch2 );	break;
						}
*/					}
				}
			}
		}

		// Play the Hurrah sound every now-and-then with some comments
		if ( Level.TimeSeconds > fTimeToCheer )
		{
			// Pick team to score; score is forced into three phases for game balance
			// reasons:  First, crank up Gryffindor's score to max as quickly as we can
			// get away with, to make point spread the max in Gryffindor's favor.  Then,
			// steadly increase Opponent score until the point spread is max in reverse.
			// After that, just maintain random scoring at worst point spread.
			if ( GryffScore < 140 )
			{
				if ( OpponentScore >= 20 || fRand() < 0.95 )
					eTeam = TA_Gryffindor;
				else
					eTeam = TA_Opponent;
				fTimeToCheer = Level.TimeSeconds + 4.0 + 1.0*FRand();
			}
			else if ( OpponentScore < 280 )
			{
				if ( GryffScore >= 160 || fRand() < 0.95 )
					eTeam = TA_Opponent;
				else
					eTeam = TA_Gryffindor;
				fTimeToCheer = Level.TimeSeconds + 4.0 + 2.0*FRand();
			}
			else
			{
				if ( OpponentScore - GryffScore < 140 )
					eTeam = TA_Opponent;
				else
					eTeam = TA_Gryffindor;
				fTimeToCheer = Level.TimeSeconds + 6.0 + 3.0*FRand();
			}

			switch ( eTeam )
			{
				case TA_Gryffindor:
					GryffScore += 10;
					ScoreBoard.SetGryffindorScore( GryffScore );
					break;

				case TA_Opponent:
					OpponentScore += 10;
					ScoreBoard.SetOpponentScore( OpponentScore );
					break;
			}

			if ( Crowds[ eTeam ] != None )
				Crowds[ eTeam ].Cheer();

			fProximity = VSize( Harry.Location - Snitch.Location );		// Ignore tracking offset
			if ( Commentator != None && ( fProximity > fSnitchNeutralRadiusAt0 ) )	// If not focusing on Harry
			{
				switch ( Rand(4) )
				{
					case 0:
						switch ( eTeam )
						{
							case TA_Gryffindor:	Commentator.SayComment( QC_HasQuaffle, TA_Gryffindor );	break;
							case TA_Opponent:	Commentator.SayComment( QC_HasQuaffle, TA_Opponent );	break;
						}
						break;

					case 1:
						if ( Commentator.CommentHasBeenSaidBefore( QC_HasQuaffle ) )
						{
							switch ( eTeam )
							{
								case TA_Gryffindor:	Commentator.SayComment( QC_Scores, TA_Gryffindor );	break;
								case TA_Opponent:	Commentator.SayComment( QC_Scores, TA_Opponent );	break;
							}
						}
						break;

					case 2:
						Commentator.SayComment( QC_KeeperDives );
						break;

					case 3:
						Commentator.SayComment( QC_Block );
						break;
				}
			}
		}


		// See if snitch has become visible yet; comment on it
		if ( Snitch.bHidden )
			bSnitchVisible = false;
		else
		{
			if ( !bSnitchVisible && (Commentator == None || Commentator.SayComment( QC_TheresTheSnitch )) )
				bSnitchVisible = true;
		}

		// See if other seeker has joined the pursuit of the snitch; comment on it
		if ( Snitch.bHidden || Seeker == None )
			bSeekerJoinedPursuit = false;
		else
		{
			fProximity = VSize( Seeker.Location - Snitch.Location );
			if ( !bSeekerJoinedPursuit )
			{
				if ( fProximity < 500 )
				{
					if ( Commentator == None || Commentator.SayComment( QC_HereComesSeeker, TA_Opponent ) )
						bSeekerJoinedPursuit = true;
				}
			}
			else
			{
				if ( fProximity > 1200 )
					bSeekerJoinedPursuit = false;
			}
		}

		// Test: Play random comment (ones not used anywhere yet)
/*
		switch ( Rand(23) )
		{
			case 11:	Commentator.SayComment( QC_BludgerPursuit );		break;
			case 12:	Commentator.SayComment( QC_BludgerPursuit_Multi );	break;
			case 13:	Commentator.SayComment( QC_BludgerMiss );			break;
			case 14:	Commentator.SayComment( QC_BludgerHit );			break;

			case 15:	Commentator.SayComment( QC_HitNearDeath );			break;
			case 16:	Commentator.SayComment( QC_HitDying );				break;

			case 19:	Commentator.SayComment( QC_ReturnToFlight );		break;

			case 22:	Commentator.SayComment( QC_Foul );					break;
		}
*/

	}

	function OnTouchEvent( Pawn Subject, Actor Object )
	{
		// Something touched something, and the event affects the flow of the
		// mini-game.  Update game state accordingly.
		local BroomHoop Hoop;

		// If Harry touched something...
		if ( Subject == Harry )
		{
			switch ( PlayMechanic )
			{
				case PM_Hoops:
					Hoop = BroomHoop( Object );
					if ( Hoop != None )
					{
						// Harry touched a hoop in snitches hoop trail; let trail react to it
						Snitch.HoopTrail.OnHoopTouch( Hoop );
					}
					else if ( Object == Snitch )
					{
						// Harry grazed the snitch; show him missing a catch (if he didn't really catch it)
						if ( !Snitch.bHidden && Snitch.HoopTrail.HoopsToGo > 0 )
						{
							Harry.PlayAnim( 'Miss' );
						}
					}
					else
					{
						// Harry touched something else
						PlayerHarry.ClientMessage( "Touched "$Object.Tag );
					}
					break;

				case PM_Proximity:
				case PM_ProximityWithHoops:
					if ( Object == Snitch )
					{
						// Harry grazed the snitch; show him missing a catch (if he didn't really catch it)
						if ( !Snitch.bHidden && fProgressPercent < 90.0 )
						{
//							Harry.PlayAnim( 'Miss' );
						}
					}
					else
					{
						// Harry touched something else
						PlayerHarry.ClientMessage( "Touched "$Object.Tag );
					}
					break;
			}
		}
		else
		{
			// Unexpected touch event
			Super.OnTouchEvent( Subject, Object );
		}
	}

	function OnBumpEvent( Pawn Subject, Actor Object )
	{
		// Something bumped something, and the event affects the flow of the
		// mini-game.  Update game state accordingly.

		// If Harry bumped something...
		if ( Subject == Harry )
		{
			// Deduct progress by setting back track distance
			if ( Level.TimeSeconds > fPenaltyGraceExpiration )
			{
				fSnitchTrackDist += 0.20 * (SnitchTrackDistMax - SnitchTrackDistMin);
				if ( fSnitchTrackDist > SnitchTrackDistMax )
					fSnitchTrackDist = SnitchTrackDistMax;
				fPenaltyGraceExpiration = Level.TimeSeconds + 1.0;
			}
		}
		else
		{
			// Unexpected bump event
			Super.OnBumpEvent( Subject, Object );
		}
	}

	function OnTriggerEvent( Actor Other, Pawn EventInstigator )
	{
		// Something triggered a 'Director' event.
		local Bludger	Bludger;

		// If Other is the HoopTrail, then it's telling the director that the
		// game progress has changed
		if ( PlayMechanic == PM_Hoops && Other == Snitch.HoopTrail )
		{
			// Update the progress element of the HUD
			ProgressBar.SetProgress( 100 * (Snitch.HoopTrail.HoopsToHit - Snitch.HoopTrail.HoopsToGo) / Snitch.HoopTrail.HoopsToHit );

//			if ( Snitch.HoopTrail.HoopsToGo <= 4 )	// *** Test for winning ***
			if ( Snitch.HoopTrail.HoopsToGo <= 0 )
			{
				// Hit all the hoops!  Caught the Snitch!
				// Turn off hoop trail and put snitch in harry's hand
				Harry.bAuxBoost = false;
				Harry.SetLookForTarget( None );
				Harry.SecondaryAnim = 'Hold';	// This will become the loop animation after the 'Catch' finishes
				Harry.PlayAnim( 'Catch' );

				Snitch.StopFlyingOnPath();
				Snitch.HoopTrail.GotoState( 'TrailOff' );
				Snitch.SetLocation( Harry.WeaponLoc );
				Snitch.SetOwner( Harry );
				Snitch.SetPhysics( PHYS_Trailer );

				GotoState( 'GameWon' );
			}

			// If Hoop Trail progress suggests a change in speed, tell Harry
			Harry.bAuxBoost = Snitch.HoopTrail.bSpeedBoostSuggested;
		}
		else if ( Other == Seeker )
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
			Harry.SetLookForTarget( None );
			Harry.SetKickTargetClass( '' );
			CameraTarget.SetLookForTarget( Harry );

			// Make bludgers stop seeking Harry
			foreach AllActors( class'Bludger', Bludger )
				Bludger.SeekTarget( None );

			GotoState( 'GameLost' );
		}
		else
		{
			// Unexpected trigger event
			Super.Trigger( Other, EventInstigator );
		}
	}

	function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
	{
		local string			sActualCommand;

		sActualCommand = ParseDelimitedString( Command, " ", 1, false );

		if( sActualCommand ~= "ContinueGame" )
		{
			// Trench transition CutScene ended; continue playing quidditch
			bInTrench = true;

			CutCue( Cue );
			return true;
		}
		else
			return Global.CutCommand( Command, Cue, bFastFlag );
	}

	function OnActionKeyPressed()
	{
		local float		fProximity;
		local Bludger	Bludger;

		// Called when the player's "Action" key/button is pressed.
		Super.OnActionKeyPressed();

		// Catch snitch if harry is reaching for it, and it's close enough; goto Won state
		if ( bCanReachForSnitch )
		{
			fProximity = VSize( Harry.Location - Snitch.Location );	// Ignore tracking offset

			if ( fProgressPercent > 98.0 /*&& fProximity < SnitchTrackDistMin + 50*/ && !(bFinalMatch && !bInTrench) )	// Can't allow a catch before getting into trench during final match
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

				// Tell seeker to stop looking for the Snitch and kicking
				if ( Seeker != None )
				{
					Seeker.SetLookForTarget( None );
					Seeker.SetKickTargetClass( '' );
				}

				// Make bludgers stop seeking Harry
				foreach AllActors( class'Bludger', Bludger )
					Bludger.SeekTarget( None );

				GotoState( 'GameWon' );
			}
			else
			{
				Harry.PlayAnim( 'Miss', , 0.1 );
				if ( Commentator != None )
					Commentator.SayComment( QC_MissedSnitch, , true );
			}
		}
	}

	event Timer()
	{
		// Time to go into trench; trigger transition cutscene
		PlayerHarry.ClientMessage( Name$": Time to go into trench." );
		Log( Name$": Time to go into trench." );

		TriggerEvent( MatchEvents_Final.GoingIntoTrench, self, None );
	}

	function OnPlayerDying()
	{
		// Called when player starts dying.

		PlayerHarry.ClientMessage( "Player dying..." );
		GotoState( 'GameLosing' );
	}

	function OnPlayersDeath()
	{
		// Called when player dies.

		PlayerHarry.ClientMessage( "Player died." );
		GotoState( 'GameLost' );
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$" Exited "$GetStateName()$" State" );
		Log( Name$" Exited "$GetStateName()$" State" );
		if ( bFinalMatch )
			SetTimer( 0, false );
		ProgressBar.Show( false );
		SeekerHealthBar.End();

		// Reset camera to follow Harry
		Harry.Cam.SetCameraMode( CM_Standard );
		Harry.Cam.SetCameraMode( CM_Quidditch );
	}
}

state GameCatch
{
	function BeginState()
	{
		local Bludger	Bludger;

		PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
		Log( Name$" Entered "$GetStateName()$" State" );

		CatchTriesLeft = SnitchMaxCatchTries;
		SetTimer( 10.0, false );		// Watchdog timer in case Harry never catches snitch

		// Make Harry reach for snitch
		Harry.SetReaching( true );

		// Make bludgers stop seeking Harry
		foreach AllActors( class'Bludger', Bludger )
			Bludger.SeekTarget( None );

		// Tell Harry not to kick anyone
		Harry.SetKickTargetClass( '' );

		// Switch to catch-the-snitch hud element
//		QuidHud( Harry.MyHud ).PlayHUDGame(true);			/***/
//		QuidHud( Harry.MyHud ).SetHUDGameType(HUDG_QUIDDITCH);

//		Harry.Cam.SetCameraMode( CM_LockAroundHarry);		/***/
//		Harry.cam.CameraDistance = 200.000;
//		Harry.cam.TargetRot = rot(5000, 5000, 0);
	}

	function Tick( float DeltaTime )
	{
		local TeamAffiliation	eTeam;

		// Comment on Harry's pursuit of the snitch
		if ( Commentator != None )
		{

			Commentator.SayComment( QC_ClosingOnSnitch );
/*			switch ( Rand(2) )
			{
				case 0: Commentator.SayComment( QC_ClosingOnSnitch );	break;
				case 1: Commentator.SayComment( QC_ClosingOnSnitch2 );	break;
			}
*/		}

		// Play the Hurrah sound every now-and-then
		if ( Level.TimeSeconds > fTimeToCheer )
		{
			if ( Rand(2) == 0 )
				eTeam = TA_Gryffindor;
			else
				eTeam = TA_Opponent;
			Log( "Playing "$eTeam$" crowd hurrah at "$Level.TimeSeconds );
			if ( Crowds[ eTeam ] != None )
				Crowds[ eTeam ].Cheer();
			fTimeToCheer = Level.TimeSeconds + 8.0 + 3.0*FRand();
		}
	}

	function OnActionKeyPressed()
	{
		// Called when the player's "Action" key/button is pressed.
		Super.OnActionKeyPressed();

		// Determine if snitch is caught; if so, goto Won state; otherwise
		// either wait for a few more tries, or return to regular game play
		if ( true /*QuidHud( Harry.MyHud ).HUDGameGrab()*/ /*Snitch caught*/ )
		{
			// Caught the Snitch!  Put snitch in harry's hand
			if ( PlayMechanic == PM_ProximityWithHoops )
				Snitch.HoopTrail.GotoState( 'TrailOff' );
			Snitch.StopFlyingOnPath();
			Harry.CatchTarget( Snitch, 'IPHarry_Win' );
			Snitch.Halo.bHidden = true;

//			QuidHud(Harry.myHUD).DestroyPopup();	/***/

			// Tell seeker to stop looking for the Snitch and kicking
			if ( Seeker != None )
			{
				Seeker.SetLookForTarget( None );
				Seeker.SetKickTargetClass( '' );
			}

			GotoState( 'GameWon' );
		}
		else
		{
			--CatchTriesLeft;
			if ( CatchTriesLeft <= 0 )
			{
				// Turn off hud progress element
//				QuidHud( Harry.MyHud ).PlayHUDGame(false);		/***/
//				QuidHud(Harry.myHUD).DestroyPopup();
				// Set camera to trail behind seekers trailing snitch
				SetCameraToFollowSnitch();

				Harry.SetReaching( false );
				GotoState( 'GamePlay' );
			}
		}
	}

	function Timer()
	{
		// Never actioned on the snitch; go back to regular gameplay

		// Turn off hud progress element
//		QuidHud( Harry.MyHud ).PlayHUDGame(false);	/***/
//		QuidHud(Harry.myHUD).DestroyPopup();
		// Set camera to trail behind seekers trailing snitch
		SetCameraToFollowSnitch();

		Harry.SetReaching( false );
		GotoState( 'GamePlay' );
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( "Exited GameCatch State" );
		Log( "Exited GameCatch State" );

		SetTimer( 0.0, false );
//		Harry.StopFlyingOnPath();
		fProgressPercent = 75.0;
	}
}

state GameWon
{
Begin:
	bGryffWon = true;
	ComputeScore();
	
	// AE:
	PlayBigCheer();

	// Trigger fanfare
	TriggerEvent( 'FanfareMusicEnd', self, None );

	// Play the final commentator comments
	if ( Commentator != None )
	{
		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
		Commentator.SayComment( QC_Positive, , true );
		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
		Commentator.SayComment( QC_CaughtSnitch, , true );

		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
		if ( bWonCup )
		{
			Commentator.SayComment( QC_WinsCup, TA_Gryffindor, true );		// Won final match and cup
			Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
			Commentator.SayComment( QC_Positive, , true );
		}
		else
			Commentator.SayComment( QC_WinsMatch, TA_Gryffindor, true );	// Won match

		Sleep( Commentator.TimeLeftUntilSafeToSayAComment() );
		Commentator.SayComment( QC_SigningOff );
		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
	}
	else
	{
		Sleep( 12.0 );
	}

	// Take back the house points earned last time match was played
	Harry.AddGryffindorPoints( -Harry.quidGameResults[ Harry.curQuidMatchNum ].housePoints );

	// Record the results of the quidditch match
	Harry.quidGameResults[ Harry.curQuidMatchNum ].myScore = GryffScore;
	Harry.quidGameResults[ Harry.curQuidMatchNum ].opponentScore = OpponentScore;
	Harry.quidGameResults[ Harry.curQuidMatchNum ].housePoints = max(GryffScore - OpponentScore, 0);
	Harry.quidGameResults[ Harry.curQuidMatchNum ].bWon = true;

	// Add house points earned this time
	Harry.AddGryffindorPoints( Harry.quidGameResults[ Harry.curQuidMatchNum ].housePoints );

	// Notify the house point system if this is the first time this match has been played
	if ( bFirstTimeThisMatch )
		HousePoints.QuidditchUpdateHousepoints( Harry.curQuidMatchNum );

	// Trigger appropriate win cutscene; cut scene will load the next level
	if ( bFinalMatch )
	{
		TriggerEvent( MatchEvents_Final.Won, self, None );
		TriggerEventDelayed( 5.0, MatchEvents_Final.End, , EndCue );
	}
	else
	{
		PlayerHarry.ClientMessage( Name$" triggering event '"$MatchEvents.Won$"'" );
		Log( Name$" triggering event '"$MatchEvents.Won$"'" );

		TriggerEvent( MatchEvents.Won, self, None );
		TriggerEventDelayed( 5.0, MatchEvents.End, , EndCue );
	}
}

state GameLosing
{
	function BeginState()
	{
		local Bludger	Bludger;

		PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
		Log( Name$" Entered "$GetStateName()$" State" );

		// Make bludgers stop seeking Harry
		foreach AllActors( class'Bludger', Bludger )
			Bludger.SeekTarget( None );

		// Tell Harry not to kick anyone
		Harry.SetKickTargetClass( '' );
	}

	function OnPlayersDeath()
	{
		// Called when player dies.

		PlayerHarry.ClientMessage( "Player died" );
		bHarryDied = true;
		GotoState( 'GameLost' );
	}

Begin:
	if ( Commentator != None )
	{
		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
		Commentator.SayComment( QC_Dying, , true );
	}

Loop:
	Sleep( 0.1 );

	goto 'Loop';
}

state GameLost
{
Begin:
	ComputeScore();

	// Play the final commentator comments
	if ( Commentator != None )
	{
		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
		Commentator.SayComment( QC_Dead, , true );

		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
		Commentator.SayComment( QC_WinsMatch, TA_Opponent, true );		// Opponent won match

		if ( bWonCup )
		{
			Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
			Commentator.SayComment( QC_WinsCup, TA_Gryffindor, true );	// Gryff won cup anyway
		}

		Sleep( Commentator.TimeLeftUntilSafeToSayAComment() );
		Commentator.SayComment( QC_SigningOff );
		Sleep( Commentator.TimeLeftUntilSafeToSayAComment( true ) );
	}
	else
	{
		Sleep( 0.2 );
	}

	// Record the results of the quidditch match
	Harry.quidGameResults[ Harry.curQuidMatchNum ].myScore = GryffScore;
	Harry.quidGameResults[ Harry.curQuidMatchNum ].opponentScore = OpponentScore;
	Harry.quidGameResults[ Harry.curQuidMatchNum ].housePoints = max(GryffScore - OpponentScore, 0);
	Harry.quidGameResults[ Harry.curQuidMatchNum ].bWon = false;

	// Notify the house point system if this is the first time this match has been played
	if ( bFirstTimeThisMatch )
		HousePoints.QuidditchUpdateHousepoints( Harry.curQuidMatchNum );

	// Trigger appropriate lose cutscene; cut scene will load the next level
	if ( bHarryDied )
	{
		if ( bFinalMatch )
		{
			TriggerEvent( MatchEvents_Final.Died, self, None );
			TriggerEventDelayed( 5.0, MatchEvents_Final.End, , EndCue );
		}
		else
		{
			PlayerHarry.ClientMessage( Name$" triggering event '"$MatchEvents.Died$"'" );
			Log( Name$" triggering event '"$MatchEvents.Died$"'" );

			TriggerEvent( MatchEvents.Died, self, None );
			TriggerEventDelayed( 5.0, MatchEvents.End, , EndCue );
		}

		Harry.AddHealth( 50 );	// Make Harry recover a bit before next level
	}
	else
	{
		Sleep( 5.0 );	// *** Temp until cutscenes do more ***

		if ( bFinalMatch )
		{
			TriggerEvent( MatchEvents_Final.Lost, self, None );
			TriggerEventDelayed( 5.0, MatchEvents_Final.End, , EndCue );
		}
		else
		{
			PlayerHarry.ClientMessage( Name$" triggering event '"$MatchEvents.Lost$"'" );
			Log( Name$" triggering event '"$MatchEvents.Lost$"'" );

			TriggerEvent( MatchEvents.Lost, self, None );
			TriggerEventDelayed( 5.0, MatchEvents.End, , EndCue );
		}
	}
}


state PendingEvent
{
Begin:
	PlayerHarry.ClientMessage( Name$" Entered "$GetStateName()$" State" );
	Log( Name$" Entered "$GetStateName()$" State" );

	// Wait the for a specified time, give cue, trigger specified event,
	// then goto specified state

	Sleep( fDelayedEventDelayTime );

	if ( DelayedEventCue != "" )
		CutCue( DelayedEventCue );

	if ( DelayedEventName != '' )
		TriggerEvent( DelayedEventName, self, None );

	if ( DelayedEventNextState != '' )
		GotoState( DelayedEventNextState );
}

defaultproperties
{
	Tag=Director
	bNeedsCommentator=true
	InitialState=GameIntro
	HoopsToHit=3
	PlayMechanic=PM_ProximityWithHoops
	
	fTimeBeforeGoingIntoTrench=25
	bForceFinalMatch=false

	CameraTrailDist=175

	fSnitchTrackingOffset=150
	fSnitchMaxGainRadiusAt0=100
	fSnitchNeutralRadiusAt0=500
	fSnitchNeutralRadiusAt100=150
	fSnitchMaxLossRadiusAt0=1200
	fSnitchMaxGainRate=20.0
	fSnitchMaxLossRate=10.0

	fSnitchMaxCatchTime=20.0
	SnitchMaxCatchTries=3
	RandSeed=(SeedA=1.1422397692371322e33,SeedB=1.4991598592e10,SeedC=4.569592902248643e33,SeedD=1.7181077547278926e19)
}
