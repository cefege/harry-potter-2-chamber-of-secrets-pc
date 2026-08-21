//=============================================================================
// BroomHoopTrail -- A series of broom hoops that emit from a flying object
//=============================================================================
class BroomHoopTrail extends Actor;

var Harry		PlayerHarry;
var Director	Director;				// Object in charge of the rules of the current mini-game

var Actor		Emitter;				// Thing the hoop trail appears to be emitting from
var BroomHoop	Hoops[ 5 ];			// Cache of hoops
const			MaxHoops = 5;			// How many physical hoops exist

var float		fHoopSpacing;			// Seconds between hoop emissions
var int			TrailLen;				// How many hoops define touchable portion of trail
var int			InitialTrailEnd;		// How many hoops back from emitter is the initial end of trail
var bool		bHoopsVisible;			// Whether or not to display hoops
var bool		bTrackProgress;			// Whether or not hoops track player progress
var int			HoopsToHit;				// How many hoops have to be hit in a row to achieve goal

var int			NextHoopToUse;			// Which hoop in hoop array is free to be added at head of trail
var int			ValidHoops;				// How many hoops have been laid by emitter so far
var float		fBirthTimeOfNewestHoop;	// When emitter last placed a hoop
var int			TrailEnd;				// Which hoop marks the end of the current hoop trail

var int			NextHoopToHit;			// Which hoop needs to be hit next to continue increasing stage
var int			CurrentStage;			// What grade of hoops are being added to trail
var bool		bSpeedBoostSuggested;	// Hoop progress is ahead of gap closure, better speed up to close gap

var int			HoopsToGo;				// How many hoops left to be hit in a row to achieve goal

var Sound		HoopSounds[16];			// All the different through-hoop sounds


//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
	local int		i;
	local vector	Position;

	Super.PostBeginPlay();

	// Find player that channels ClientMessages
	foreach AllActors( class'Harry', PlayerHarry )
		break;

	// Find mini-game referee
	foreach AllActors( class'Director', Director )
		break;

	// Create cache of hoops
	CurrentStage = 1;
	for ( i=0; i<MaxHoops; ++i )
	{
		Position.x = (i+1)*20;
		Position.y = 0;
		Position.z = -100;

		Hoops[i] = Spawn( class'BroomHoop', None, 'Hoop', Position, rot(0,0,0) );
		Hoops[i].HoopNumber = i;
		Hoops[i].Stage = 0;	// Force appearance change at first use
//		ChangeHoopAppearance( Hoops[i], CurrentStage );
	}

	// Load the hoop sounds		/***/
/*
	HoopSounds[ 0] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop';
	HoopSounds[ 1] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop01';
	HoopSounds[ 2] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop02';
	HoopSounds[ 3] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop03';
	HoopSounds[ 4] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop04';
	HoopSounds[ 5] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop05';
	HoopSounds[ 6] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop06';
	HoopSounds[ 7] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop07';
	HoopSounds[ 8] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop08';
	HoopSounds[ 9] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop09';
	HoopSounds[10] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop10';
	HoopSounds[11] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop11';
	HoopSounds[12] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop12';
	HoopSounds[13] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop13';
	HoopSounds[14] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop14';
	HoopSounds[15] = Sound'HPSounds.Quidditch_sfx.Q_Through_Hoop15';
*/

	// Set trail to be initially off
	InitialState = 'TrailOff';
}


//-------------------------------------------------------------------------------------------
// Operational methods
//-------------------------------------------------------------------------------------------

function SetTrailProperties( float	fNewHoopSpacing,
							 int	NewTrailLen,
							 int	NewInitialTrailEnd,
							 bool	bNewHoopsVisible,
							 bool	bNewTrackProgress )
{
	fHoopSpacing	= fNewHoopSpacing;
	TrailLen		= NewTrailLen;
	InitialTrailEnd	= NewInitialTrailEnd;
	bHoopsVisible	= bNewHoopsVisible;
	bTrackProgress	= bNewTrackProgress;
}

function SetHoopsToHit( int NewHoopsToHit )
{
	HoopsToHit		= NewHoopsToHit;
}

function ChangeHoopAppearance( BroomHoop Hoop, int ParticleStage )
{
	// Changes the hoop's appearance to use the particles associated with
	// the specified stage.

	switch ( ParticleStage )
	{
		case 1:	Hoop.attachedParticleClass[0]=class'Ring1';	break;
		case 2:	Hoop.attachedParticleClass[0]=class'Ring2';	break;
		case 3:	Hoop.attachedParticleClass[0]=class'Ring3';	break;
		case 4:	Hoop.attachedParticleClass[0]=class'Ring4';	break;
		case 5:	Hoop.attachedParticleClass[0]=class'Ring5';	break;
	}

	if ( bHoopsVisible )
	{
		Hoop.changeAttachedParticleFX( Hoop.attachedParticleClass[0] );
//		PlayerHarry.ClientMessage( "Hoop "$Hoop.Name$" changed appearance to stage "$ParticleStage$"." );
//		Log( "Hoop "$Hoop.Name$" changed appearance to stage "$ParticleStage$"." );
	}
	else
	{
//		PlayerHarry.ClientMessage( "Attempt to change appearance of Hoop "$Hoop.Name$" to stage "$ParticleStage$"." );
//		Log( "Attempt to change appearance of Hoop "$Hoop.Name$" to stage "$ParticleStage$"." );
	}
}

function SetTrailEnd( int NewTrailEnd )
{
	// Makes the indicated hoop the new end of the current hoop trail by
	// making any visible hoops after the new end disappear, and making
	// sure enough hoops are visible ahead of the end to preserve the
	// designated trail length.  Will limit trail length is beginning of
	// trail extends past emitter position.
	local BroomHoop	Hoop;
	local int		HoopsToDo;
	local int		i;

//	PlayerHarry.ClientMessage("New Trail End: "$NewTrailEnd);
//	Log("New Trail End: "$NewTrailEnd);

	// Make sure all hoops outside of the trail will be invisible
	i = NewTrailEnd;
	for ( HoopsToDo = MaxHoops - TrailLen; HoopsToDo > 0; --HoopsToDo )
	{
		--i;
		if ( i < 0 )
			i += MaxHoops;
		Hoop = Hoops[i];
		if ( !(Hoop.IsInState('HoopInvisible') || Hoop.IsInState('HoopDisappearing')) )
		{
			Hoop.GotoState('HoopDisappearing');
		}
	};

	// Make sure all hoops inside the trail will be visible
	i = NewTrailEnd;
	for ( HoopsToDo = TrailLen; HoopsToDo > 0 && i != NextHoopToUse; --HoopsToDo )
	{
		Hoop = Hoops[i];
		if ( i < ValidHoops		// Make sure hoop was actually laid by current emitter
			 && (Hoop.IsInState('HoopInvisible') || Hoop.IsInState('HoopDisappearing')) )
		{
			// Make sure hoop appearance matches that for current stage
			if ( Hoop.Stage != CurrentStage )
			{
				Hoop.Stage = CurrentStage;
				ChangeHoopAppearance( Hoop, CurrentStage );
			}

			Hoop.GotoState('HoopNextToHit');
		}

		++i;
		if ( i >= MaxHoops )
			i = 0;
	};
}

function UpdateStage()
{
	// Update stage to reflect how many hoops are left to go
	local int	NewStage;

	NewStage = 5 - (4*HoopsToGo/HoopsToHit);
	if ( CurrentStage != NewStage )
		CurrentStage = NewStage;
//	PlayerHarry.ClientMessage("HoopsToGo: "$HoopsToGo);
//	Log("HoopsToGo: "$HoopsToGo);
//	PlayerHarry.ClientMessage("New Stage: "$NewStage);
//	Log("New Stage: "$NewStage);
}

function OnHoopTouch( BroomHoop Hoop )
{
	// The player touched a hoop in the hoop trail.
	// This is the default handler; it is overridden in states.
}


//-------------------------------------------------------------------------------------------
// States
//
// TrailOff		- Hoop trail is not being generated
// TrailOn		- Hoop trail is being generated
//-------------------------------------------------------------------------------------------

state TrailOff
{
	// Hide hoops
	function BeginState()
	{
		local int	i;

		for ( i=0; i<MaxHoops; ++i )
		{
			Hoops[i].GotoState( 'HoopInvisible' );
		}
	}
}

state TrailOn
{
	// Show hoops
	function BeginState()
	{
		NextHoopToUse = 0;
		ValidHoops = 0;
		TrailEnd = NextHoopToUse - InitialTrailEnd;
		if ( TrailEnd < 0 )
			TrailEnd += MaxHoops;
		SetTrailEnd( TrailEnd );
		HoopsToGo = HoopsToHit;
		NextHoopToHit = -1;
		SetTimer( fHoopSpacing, true );
	}

	function Timer()
	{
		local BroomHoop	Hoop;
		local int		HoopToShow;
		local int		EndSetBack;
		local int		OldHoopsToGo;

		// Put a hoop at current position of emitter (hidden)
		Hoop = Hoops[ NextHoopToUse ];
		Hoop.GotoState( 'HoopInvisible' );
		if ( Emitter != None )
		{
			Hoop.SetLocation( Emitter.Location );
			Hoop.SetRotation( Emitter.Rotation );
		}
		++ValidHoops;
		fBirthTimeOfNewestHoop = Level.TimeSeconds;

		++NextHoopToUse;
		if ( NextHoopToUse >= MaxHoops )
			NextHoopToUse = 0;

		// If trail end is now farther back then initial setback allows, move it up
		// to initial setback and penalize progress
		OldHoopsToGo = HoopsToGo;
		EndSetBack = NextHoopToUse-1 - TrailEnd;
		if ( EndSetBack < 0 )
			EndSetBack += MaxHoops;
		if ( EndSetBack > InitialTrailEnd )
		{
			// Move up trail end
			TrailEnd = NextHoopToUse-1 - InitialTrailEnd;
			if ( TrailEnd < 0 )
				TrailEnd += MaxHoops;
			NextHoopToHit = TrailEnd;
			SetTrailEnd( TrailEnd );

			// Penalize progress
//			Log("EndSetBack: "$EndSetBack);
			HoopsToGo += EndSetBack - InitialTrailEnd;
			if ( HoopsToGo >= HoopsToHit )
			{
				// Lost all progress; start sequence over
				NextHoopToHit = -1;
				HoopsToGo = HoopsToHit;
			}

			if ( bTrackProgress )
			{
				bSpeedBoostSuggested = false;
				UpdateStage();
			}
		}
		// If trail end was so close to emitter that the trail length had been
		// shortened, then this tick has just made the visible length grow by
		// one hoop; reset the trail so all the right hoops are visible now
		else if ( EndSetBack < TrailLen )
		{
			SetTrailEnd( TrailEnd );
		}

		// Tell game referee if progress has changed
		if ( bTrackProgress && HoopsToGo != OldHoopsToGo )
		{
			Director.Trigger( Self, None );
		}
	}

	function OnHoopTouch( BroomHoop Hoop )
	{
		// The player touched a hoop in the hoop trail; turn off hoop and
		// build-up trail progress
		local int	NewStage;
		local int	HoopsSkipped;
		local int	OldHoopsToGo;
		local int	VisibleLen;
		local int	HoopSound;

		if ( !(Hoop.IsInState('HoopInvisible') || Hoop.IsInState('HoopDisappearing')) )
		{
			// Hit a living hoop...
//			PlayerHarry.ClientMessage("Hit Hoop "$Hoop.HoopNumber);
//			Log("Hit Hoop "$Hoop.HoopNumber);
			OldHoopsToGo = HoopsToGo;
			--HoopsToGo;

			if ( Hoop.HoopNumber == NextHoopToHit )
			{
				// Hit the right hoop, limit increased progress
				if ( HoopsToGo < 0 )
					HoopsToGo = 0;
			}
			else
			{
				if ( NextHoopToHit != -1 )
				{
					// Skipped over some hoops; set back progress
					HoopsSkipped = Hoop.HoopNumber - NextHoopToHit;
					if ( HoopsSkipped < 0 )
						HoopsSkipped += MaxHoops;
					HoopsToGo += HoopsSkipped;
					if ( HoopsToGo >= HoopsToHit )
						HoopsToGo = HoopsToHit-1;
				}
			}

			// Move trail end to the next hoop to hit
			NextHoopToHit = Hoop.HoopNumber + 1;
			if ( NextHoopToHit >= MaxHoops )
				NextHoopToHit = 0;
			TrailEnd = NextHoopToHit;

			// Limit hoop progress to number of hoops left to close gap
			// with emitter
			VisibleLen = NextHoopToUse - TrailEnd;
			if ( VisibleLen < 0 )
				VisibleLen += MaxHoops;
			if ( HoopsToGo < VisibleLen )
			{
				HoopsToGo = VisibleLen;
				bSpeedBoostSuggested = true;
			}
			else
				bSpeedBoostSuggested = false;

			// Play hoop sound
			HoopSound = 15 - HoopsToGo;
			if ( HoopSound < 1 )
				HoopSound = 1;
			Hoop.PlaySound( HoopSounds[ HoopSound ] );

			// Update trail
			UpdateStage();
			SetTrailEnd( TrailEnd );

			// Tell game referee if progress has changed
			if ( HoopsToGo != OldHoopsToGo )
			{
				if ( HoopsToGo == 0 )
					PlayerHarry.ClientMessage("Hit all the hoops!");

				Director.Trigger( Self, None );
			}
		}
	}

	function EndState()
	{
		SetTimer( 0.0, false );
		bSpeedBoostSuggested = false;

		// If not won, all progress is lost
		if ( bTrackProgress && HoopsToGo != 0 && HoopsToGo != HoopsToHit )
		{
			HoopsToGo = HoopsToHit;
			Director.Trigger( Self, None );
		}
	}
}

defaultproperties
{
	bHidden=true
	fHoopSpacing=1.0
	TrailLen=3
	InitialTrailEnd=3
	bHoopsVisible=false
	bTrackProgress=false

	bCollideActors=false
	bCollideWorld=false
	bBlockActors=false
	bBlockPlayers=false

	HoopsToHit=10
}
