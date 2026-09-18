/*=============================================================================

=============================================================================*/

struct FSoundSet
{
	USound* Sound;

	float	Pitch;
	float	PitchVar;
	float	Volume;
	float	VolumeVar;
};

#if !__GNUC__
enum ESoundSlot;
#endif

struct FCreatureSoundGroup
{
	USound*			Sounds[10];
	int				NumSounds;
#if __GNUC__	// GCC doesn't support forward declaring enums
	int				Slot;
#else
	ESoundSlot		Slot;
#endif

	float			Pitch;
	float			PitchVar;
	float			Volume;
	float			VolumeVar;
	float			Radius;
};

struct FFootStepSoundProperties
{
	USound* Sound1;
	USound* Sound2;
	USound* Sound3;
	USound* Sound4;

	float	Pitch;
	float	PitchVar;
	float	Volume;
	float	VolumeVar;
};

struct FFootSoundEntry
{
	FFootStepSoundProperties	FootStep;
	FSoundSet					Land;
	FSoundSet					Scuff;
};

struct FImpactSoundProperties
{
	float	O_Pitch;
	float	O_PitchVar;
	float	O_Volume;
	float	O_VolumeVar;

	USound* Sound1;
	USound* Sound2;
	USound* Sound3;

	float	T_Pitch;
	float	T_PitchVar;
	float	T_Volume;
	float	T_VolumeVar;
};

struct FImpactSoundEntry
{
	FImpactSoundProperties	Impact;
	FSoundSet				Slide;
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
