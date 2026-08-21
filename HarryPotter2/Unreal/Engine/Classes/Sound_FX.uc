//=============================================================================
// Sound_FX: Receives trigger messages and plays a sound file
//=============================================================================
class Sound_FX extends SpecialEvent;

#exec Texture Import File=..\engine\Textures\Snd_FX.pcx Name=Snd_FX Mips=Off Flags=2

defaultproperties
{
     InitialState=PlaySoundEffect
     Style=STY_Masked
     texture=Snd_FX
}
