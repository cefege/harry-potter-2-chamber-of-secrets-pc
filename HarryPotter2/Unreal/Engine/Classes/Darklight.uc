//=============================================================================
// A Dark light source.  for adding shadows under trees and negative lighting to spaces
//=============================================================================
class Darklight extends Light;

#exec Texture Import File=..\engine\Textures\DarkL.pcx Name=darklit Mips=Off Flags=2

defaultproperties
{
     Style=STY_Masked
     Texture=Texture'Engine.darklit'
     LightRadius=4
     LightRadiusInner=255
     bDarkLight=True
}
