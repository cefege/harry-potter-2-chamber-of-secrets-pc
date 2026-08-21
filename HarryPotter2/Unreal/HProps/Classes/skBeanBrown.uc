//===============================================================================
//  [skBeanBrown] 
//===============================================================================

class skBeanBrown extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBeanBrownMesh MODELFILE=models\skBeanBrown.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBeanBrownMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBeanBrownAnims ANIMFILE=models\skBeanBrown.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBeanBrownMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBeanBrownMesh ANIM=skBeanBrownAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBeanBrownAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBeanBrownTex0  FILE=TEXTURES\BeanBrownish_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBeanBrownMesh NUM=0 TEXTURE=skBeanBrownTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: BeanBrownish_64.bmp  Path: C:\Harry Potter 2\ART\Objects\Food_Candy\Jellybeans 


defaultproperties
{
    Mesh=skBeanBrownMesh
    DrawType=DT_Mesh
    bStatic=False
}

