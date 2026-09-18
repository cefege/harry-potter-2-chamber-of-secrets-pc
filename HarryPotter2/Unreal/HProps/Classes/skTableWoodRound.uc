//===============================================================================
//  [skTableWoodRound] 
//===============================================================================

class skTableWoodRound extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableWoodRoundMesh MODELFILE=models\skTableWoodRound.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableWoodRoundMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableWoodRoundAnims ANIMFILE=models\skTableWoodRound.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableWoodRoundMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableWoodRoundMesh ANIM=skTableWoodRoundAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableWoodRoundAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableWoodRoundTex0  FILE=TEXTURES\hogroundtable_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableWoodRoundMesh NUM=0 TEXTURE=skTableWoodRoundTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: hogroundtable_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Round Table 


defaultproperties
{
    Mesh=skTableWoodRoundMesh
    DrawType=DT_Mesh
    bStatic=False
}

