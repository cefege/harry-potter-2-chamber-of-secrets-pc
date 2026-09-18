//===============================================================================
//  [skBeanDkBlue] 
//===============================================================================

class skBeanDkBlue extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBeanDkBlueMesh MODELFILE=models\skBeanDkBlue.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBeanDkBlueMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBeanDkBlueAnims ANIMFILE=models\skBeanDkBlue.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBeanDkBlueMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBeanDkBlueMesh ANIM=skBeanDkBlueAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBeanDkBlueAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBeanDkBlueTex0  FILE=TEXTURES\BeanDkBlue.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBeanDkBlueMesh NUM=0 TEXTURE=skBeanDkBlueTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: BeanDkBlue.bmp  Path: C:\Harry Potter 2\ART\Objects\Food_Candy\Jellybeans 


defaultproperties
{
    Mesh=skBeanDkBlueMesh
    DrawType=DT_Mesh
    bStatic=False
}

