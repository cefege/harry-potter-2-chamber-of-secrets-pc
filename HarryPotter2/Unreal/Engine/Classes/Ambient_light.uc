//=============================================================================
// An ambient light source setting.
//=============================================================================
class Ambient_light extends Light;

#exec Texture Import File=..\engine\Textures\amb_light.pcx Name=amb_light Mips=Off Flags=2


defaultproperties
{
     Style=STY_Masked
     Texture=amb_light
     LightBrightness=16
     LightHue=32
     LightSaturation=200
     LightRadiusInner=150
     LightSource=LD_Ambient
}
