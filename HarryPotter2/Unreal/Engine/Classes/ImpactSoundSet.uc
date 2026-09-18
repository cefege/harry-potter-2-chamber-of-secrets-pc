//=============================================================================
// ImpactSoundSet.
//=============================================================================
class ImpactSoundSet expands SoundContainer
	native;

// Impact Matrix stuff
struct ImpactSoundProperties
{
	var() float	O_Volume;
	var() float	O_VolumeVar;
	var() float	O_Pitch;
	var() float	O_PitchVar;

	var() sound	Sound1;
	var() sound	Sound2;
	var() sound	Sound3;

	var() float	T_Volume;
	var() float	T_VolumeVar;
	var() float	T_Pitch;
	var() float	T_PitchVar;
};

struct ImpactSoundEntry
{
	var() ImpactSoundProperties Impact;
	var() SoundSet Slide;
};

var() ImpactSoundEntry Default;
var() ImpactSoundEntry Glass;
var() ImpactSoundEntry Water;
var() ImpactSoundEntry Leaves;
var() ImpactSoundEntry Snow;
var() ImpactSoundEntry Grass;
var() ImpactSoundEntry Organic;
var() ImpactSoundEntry Carpet;
var() ImpactSoundEntry Earth;
var() ImpactSoundEntry Sand;
var() ImpactSoundEntry WoodHollow;
var() ImpactSoundEntry WoodSolid;
var() ImpactSoundEntry Stone;
var() ImpactSoundEntry Metal;
var() ImpactSoundEntry Extra1;
var() ImpactSoundEntry Extra2;
var() ImpactSoundEntry Extra3;
var() ImpactSoundEntry Extra4;
var() ImpactSoundEntry Extra5;
var() ImpactSoundEntry Extra6;

defaultproperties
{
}
