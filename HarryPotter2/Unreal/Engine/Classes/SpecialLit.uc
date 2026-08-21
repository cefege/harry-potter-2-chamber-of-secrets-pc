//=============================================================================
// A special lit light source.
//=============================================================================
class Speciallit extends Light;

#exec Texture Import File=..\engine\Textures\special_lit.pcx Name=special_lit Mips=Off Flags=2

defaultproperties
{
     Style=STY_Masked
     Texture=special_lit
     LightHue=32
     LightSaturation=175
     LightRadius=32
     LightRadiusInner=180
     bSpecialLit=True
}
