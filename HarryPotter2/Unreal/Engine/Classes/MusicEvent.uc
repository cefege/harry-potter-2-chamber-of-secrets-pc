//=============================================================================
// MusicEvent.
//=============================================================================
class MusicEvent extends Triggers;

#exec Texture Import File=..\engine\Textures\musicicon.pcx Name=musicicon Mips=Off Flags=2

// Variables.
var() string           Song;
var() byte             SongSection;
var() byte             CdTrack;
var() EMusicTransition Transition;
var() bool             bSilence;
var() bool             bOnceOnly;
var() bool             bAffectAllPlayers;

// Music boost parameters
var (MusicEventBoost) bool bDoBoost;
var (MusicEventBoost) byte  PercentMusicVolume;
var (MusicEventBoost) float BoostTime;
var (MusicEventBoost) EMusicTransition EndTransition;

// Music as trigger parameters
var(MusicEventTrigger) bool bPlayOnceOnly;
var(MusicEventTrigger) name TriggerToSendWhenDone;


var byte PrevPercentMusicVolume;
var float TimeSoFar;
var PlayerPawn P;
var float MusicFade;


// When gameplay starts.
function BeginPlay()
{
}

// When triggered.
function Trigger( actor Other, pawn EventInstigator )
{
}


function CancelEvent ()
{
}


function SetMusic ()
{
}


function GotoTriggeredState (bool bBoostDone)
{
}

auto state untriggered
{
}


state BoostTriggered
{
	function beginState ()
	{
	}

	function Tick( float DeltaTime )
	{
	}
}


state WaitForSongToFinish
{
	function Tick( float DeltaTime )
	{
	}
}

defaultproperties
{
}
