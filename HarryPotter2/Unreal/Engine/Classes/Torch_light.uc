//=============================================================================
// A Torch light setting.
// for small light sources such as torches and candles
//=============================================================================
class Torch_light extends Light;

#exec Texture Import File=..\engine\Textures\T_light.pcx Name=T_light Mips=Off Flags=2

defaultproperties
{
     Style=STY_Masked
     Texture=T_light
     LightBrightness=96
     LightHue=34
     LightSaturation=50
     LightRadius=6
     LightRadiusInner=150
}
