//=============================================================================
// A Fog Light, must be in a ZoneInfo w/bFogZone=True.
//=============================================================================
class Foglight extends Light;

#exec Texture Import File=..\engine\Textures\fog_light.pcx Name=fog_light Mips=Off Flags=2

defaultproperties
{
     Rotation=(Pitch=-16448)
     bDirectional=True
     Style=STY_Masked
     Texture=fog_light
     LightBrightness=60
     LightHue=180
     LightSaturation=120
     LightRadius=16
     VolumeRadius=20
     VolumeFog=120
}
