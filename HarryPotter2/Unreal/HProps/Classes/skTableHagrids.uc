//===============================================================================
//  [skTableHagrids] 
//===============================================================================

class skTableHagrids extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableHagridsMesh MODELFILE=models\skTableHagrids.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableHagridsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableHagridsAnims ANIMFILE=models\skTableHagrids.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableHagridsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableHagridsMesh ANIM=skTableHagridsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableHagridsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableHagridsTex0  FILE=TEXTURES\hagtable_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skTableHagridsTex1  FILE=TEXTURES\hagtable_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableHagridsMesh NUM=0 TEXTURE=skTableHagridsTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skTableHagridsMesh NUM=1 TEXTURE=skTableHagridsTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: hagtable_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Hagrids Table 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: hagtable_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Hagrids Table 


defaultproperties
{
    Mesh=skTableHagridsMesh
    DrawType=DT_Mesh
    bStatic=False
}

