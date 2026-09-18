//================================================================================
// QuidditchCrowd  -- A group of Quidditch spectators sitting in one place (a tower)
//================================================================================
class QuidditchCrowd extends AmbientSound;

enum HouseAffiliation
{
	HA_Gryffindor,
	HA_Ravenclaw,
	HA_Hufflepuff,
	HA_Slytherin,
	HA_Neutral,		// Not affilated with one specific house, like faculty crowd
};

var(Crowd) HouseAffiliation	Affiliation;	// Which house is this crowd affiliated with

var(Sound) byte		CheerVolume;			// How loud to play cheers (and other outbursts)
var(Sound) byte		CheerPitch;				// What pitch to play cheers at
var(Sound) byte		CheerRadius;			// How far out cheers should be heard

var QuidditchCrowd	NextCrowd;				// Chain of crowds with similar team affilation

const				NUM_CHEER_SOUNDS = 4;
var Sound			CheerSounds[4];			// All the different cheering sounds

//-------------------------------------------------------------------------------------------
// PreBeginPlay()
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

	// Load the cheer sounds	/***/
/*
	CheerSounds[0] = Sound'HPSounds.Quidditch_sfx.Q_Hurrah1';
	CheerSounds[1] = Sound'HPSounds.Quidditch_sfx.Q_Hurrah2';
	CheerSounds[2] = Sound'HPSounds.Quidditch_sfx.Q_Hurrah3';
	CheerSounds[3] = Sound'HPSounds.Quidditch_sfx.Q_Hurrah4';
*/
}


//-------------------------------------------------------------------------------------------
// Operational methods
//-------------------------------------------------------------------------------------------

function Cheer()
{
	// Emit a cheer from all crowds in this chain of crowds

	PlaySound(
		CheerSounds[ Rand(4) ],
		SLOT_Misc,
		CheerVolume/255.0,
		,
		25.0 * (int(CheerRadius)+1),
		CheerPitch/64.0
	);

	if ( NextCrowd != None )
		NextCrowd.Cheer();
}


defaultproperties
{
	Affiliation=HA_Neutral
	AmbientSound=Sound'HPSounds.Quidditch_sfx.AMB_Crowd_Loop1'
	SoundVolume=190
	SoundPitch=64
	SoundRadius=24
	CheerVolume=255
	CheerPitch=64
	CheerRadius=200
}
