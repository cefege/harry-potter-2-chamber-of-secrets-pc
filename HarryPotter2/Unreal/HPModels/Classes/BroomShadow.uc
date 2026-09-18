//===============================================================================
//  [BroomShadow] 
//===============================================================================

#exec TEXTURE IMPORT FILE=Textures\BroomShadow.bmp NAME=BroomShadowT

class BroomShadow extends ActorShadow;

defaultproperties
{
	Texture=texture'BroomShadowT'
    MultiDecalLevel=1
	ShadowSizeFactor=.9
}

