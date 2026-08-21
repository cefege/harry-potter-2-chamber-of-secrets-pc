//===============================================================================
//  [skBeanYellowy] 
//===============================================================================

class skBeanYellowy extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBeanYellowyMesh MODELFILE=models\skBeanYellowy.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBeanYellowyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBeanYellowyAnims ANIMFILE=models\skBeanYellowy.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBeanYellowyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBeanYellowyMesh ANIM=skBeanYellowyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBeanYellowyAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBeanYellowyTex0  FILE=TEXTURES\BeanYellow_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBeanYellowyMesh NUM=0 TEXTURE=skBeanYellowyTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: BeanYellow_64.bmp  Path: C:\Harry Potter 2\ART\Objects\Food_Candy\Jellybeans 


defaultproperties
{
    Mesh=skBeanYellowyMesh
    DrawType=DT_Mesh
    bStatic=False
}

