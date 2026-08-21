//===============================================================================
//  [skTableWoodSquare] 
//===============================================================================

class skTableWoodSquare extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableWoodSquareMesh MODELFILE=models\skTableWoodSquare.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableWoodSquareMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableWoodSquareAnims ANIMFILE=models\skTableWoodSquare.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableWoodSquareMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableWoodSquareMesh ANIM=skTableWoodSquareAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableWoodSquareAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableWoodSquareTex0  FILE=TEXTURES\hogsquaretable_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableWoodSquareMesh NUM=0 TEXTURE=skTableWoodSquareTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: hogsquaretable_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Square Table 


defaultproperties
{
    Mesh=skTableWoodSquareMesh
    DrawType=DT_Mesh
    bStatic=False
}

