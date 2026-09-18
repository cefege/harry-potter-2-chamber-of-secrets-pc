//=============================================================================
// Candleflametexture.
//=============================================================================
class candleflame extends Light;

#exec Texture Import File=..\engine\Textures\Can_F.pcx Name=Can_F Mips=Off Flags=2

defaultproperties
{
     bHidden=False
     Style=STY_Translucent
     Texture=Texture'Engine.Can_F'
     LightType=LT_None
     LightBrightness=0
}
