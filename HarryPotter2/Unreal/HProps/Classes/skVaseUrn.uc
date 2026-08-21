//===============================================================================
//  [skVaseUrn] 
//===============================================================================

class skVaseUrn extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skVaseUrnMesh MODELFILE=models\skVaseUrn.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skVaseUrnMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skVaseUrnAnims ANIMFILE=models\skVaseUrn.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skVaseUrnMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skVaseUrnMesh ANIM=skVaseUrnAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skVaseUrnAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skVaseUrnTex0  FILE=TEXTURES\HWvaseTW_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skVaseUrnMesh NUM=0 TEXTURE=skVaseUrnTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: HWvaseTW_128.bmp  Path: C:\Harry Potter\ART\Objects\Vases\Gray w Handles 


defaultproperties
{
    Mesh=skVaseUrnMesh
    DrawType=DT_Mesh
    bStatic=False
}

