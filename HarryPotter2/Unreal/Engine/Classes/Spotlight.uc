//=============================================================================
// A directional spotlight.
//=============================================================================
class Spotlight extends Light;

#exec Texture Import File=..\engine\Textures\spot_light.pcx Name=spot_light Mips=Off Flags=2

defaultproperties
{
     Rotation=(Pitch=-16448)
     bDirectional=True
     Style=STY_Masked
     Texture=spot_light
     LightEffect=LE_Spotlight
     LightBrightness=96
     LightHue=32
     LightSaturation=100
     LightRadius=16
     LightRadiusInner=200
     LightCone=64
}
