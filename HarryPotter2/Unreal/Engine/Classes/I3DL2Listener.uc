//=============================================================================
// I3DL2Listener: Base class for I3DL2 room effects.
//=============================================================================

class I3DL2Listener extends Object
	abstract
	native;


var()			float		EnvironmentSize;
var()			float		EnvironmentDiffusion;
var()			int			Room;
var()			int			RoomHF;
var()			float		DecayTime;
var()			float		DecayHFRatio;
var()			int			Reflections;
var()			float		ReflectionsDelay;
var()			int			Reverb;
var()			float		ReverbDelay;
var()			float		RoomRolloffFactor;
var()			float		AirAbsorptionHF;
var()			bool		bDecayTimeScale;
var()			bool		bReflectionsScale;
var()			bool		bReflectionsDelayScale;
var()			bool		bReverbScale;
var()			bool		bReverbDelayScale;
var()			bool		bDecayHFLimit;


var	transient	int			Environment;
var transient	int			Updated;


defaultproperties
{
//	Texture=S_Emitter
	EnvironmentSize=7.5;
	EnvironmentDiffusion=1.0;
	Room=-1000;
	RoomHF=-100;
	DecayTime=1.49;
	DecayHFRatio=0.83;
	Reflections=-2602;
	ReflectionsDelay=0.007;
	Reverb=200;
	ReverbDelay=0.011;
	RoomRolloffFactor=0.0;
	AirAbsorptionHF=-5;
	bDecayTimeScale=true;
	bReflectionsScale=true;
	bReflectionsDelayScale=true;
	bReverbScale=true;
	bReverbDelayScale=true;
	bDecayHFLimit=true;
}