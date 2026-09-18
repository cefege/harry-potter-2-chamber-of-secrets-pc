//===============================================================================
//  [skHutchHagrid] 
//===============================================================================

class skHutchHagrid extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skHutchHagridMesh MODELFILE=models\skHutchHagrid.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHutchHagridMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHutchHagridAnims ANIMFILE=models\skHutchHagrid.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHutchHagridMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHutchHagridMesh ANIM=skHutchHagridAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHutchHagridAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHutchHagridTex0  FILE=TEXTURES\haghutch_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHutchHagridTex1  FILE=TEXTURES\haghutch_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHutchHagridMesh NUM=0 TEXTURE=skHutchHagridTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHutchHagridMesh NUM=1 TEXTURE=skHutchHagridTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: haghutch_128.bmp  Path: C:\Harry Potter\ART\Objects\Cabinets_Hutches_Bookcases\Hagrids Hutch 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: haghutch_128.bmp  Path: C:\Harry Potter\ART\Objects\Cabinets_Hutches_Bookcases\Hagrids Hutch 


defaultproperties
{
    Mesh=skHutchHagridMesh
    DrawType=DT_Mesh
    bStatic=False
}

