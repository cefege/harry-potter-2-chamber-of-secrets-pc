class MusicTrigger extends Triggers;

#exec Texture Import File=..\engine\Textures\mu_IconNew.pcx Name=mu_icon Mips=Off Flags=2

var()				string		Song;
var()				float		FadeInTime;
var()				float		FadeOutTime;
var()				bool		FadeOutAllSongs;

var		transient	bool		Triggered;
var 	transient	int			SongHandle;

function Trigger( Actor Other, Pawn EventInstigator )
{
	if( FadeOutAllSongs )
	{
		EventInstigator.StopAllMusic( FadeOutTime );
	}
	else
	{
		if( !Triggered )
		{
			Triggered	= true;
			SongHandle	= EventInstigator.PlayMusic( Song, FadeInTime );
		}
		else
		{
			Triggered	= false;
			if( SongHandle != 0 )
			{
				EventInstigator.StopMusic( SongHandle, FadeOutTime );
			}
			else
			{
				Log("WARNING: invalid song handle");
			}
		}
	}
}

defaultproperties
{
	bCollideActors=False
	Texture=mu_icon
}