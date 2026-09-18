
class NewMusicTrigger extends trigger;


var()				string		Song;
var()				float		FadeInTime;
var()				float		FadeOutTime;
var()				bool		FadeOutAllSongs;

var		transient	bool		Triggered;
var 	transient	int			SongHandle;

var   Harry		playerHarry;

//*******************************************************************************
function PostBeginPlay()
{
	ForEach AllActors(class'Harry', playerHarry)
		break;
}


//*******************************************************************************
function Activate( actor Other, pawn Instigator )
{
	ProcessTrigger();
}

function ProcessTrigger()
{
	local NewMusicTrigger nmt;
	if( FadeOutAllSongs )
	{
		ForEach AllActors(class'NewMusicTrigger', nmt)
			nmt.Triggered = false;

		StopAllMusic( FadeOutTime );
	}
	else
	{
		if( !Triggered )
		{
			Triggered	= true;
			SongHandle	= PlayMusic( Song, FadeInTime );

			//SoundDuration = GetSoundDuration(snd);
		}
	}
}

//*****************************************************************************
defaultproperties
{
	Texture=mu_icon
}