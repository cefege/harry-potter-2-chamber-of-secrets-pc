//=============================================================================
// SoundContainer.
//=============================================================================
class SoundContainer expands Object
	native;

// General SoundSet struct
struct SoundSet
{
	var() sound Sound;

	var() float	Pitch;
	var() float	PitchVar;
	var() float	Volume;
	var() float VolumeVar;
};

struct CreatureSoundGroup
{
	var() sound				Sound0;
	var() sound				Sound1;
	var() sound				Sound2;
	var() sound				Sound3;
	var() sound				Sound4;
	var() sound				Sound5;
	var() sound				Sound6;
	var() sound				Sound7;
	var() sound				Sound8;
	var() sound				Sound9;

	var() int				NumSounds;
//	var() Actor.ESoundSlot	Slot;

	var() float				Pitch;
	var() float				PitchVar;
	var() float				Volume;
	var() float				VolumeVar;
	var() float				Radius;
};

defaultproperties
{
}
