//===============================================================================
//  [skBoomSlang] 
//===============================================================================

class skBoomSlang extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBoomSlangMesh MODELFILE=models\skBoomSlang.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBoomSlangMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBoomSlangAnims ANIMFILE=models\skBoomSlang.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBoomSlangMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBoomSlangMesh ANIM=skBoomSlangAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBoomSlangAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBoomSlangTex0  FILE=TEXTURES\ssBoomslang.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBoomSlangTex1  FILE=TEXTURES\ssBoomslang.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBoomSlangMesh NUM=0 TEXTURE=skBoomSlangTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBoomSlangMesh NUM=1 TEXTURE=skBoomSlangTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: ssBoomslang.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Boomslang Skin 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: ssBoomslang.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Boomslang Skin 


defaultproperties
{
    Mesh=skBoomSlangMesh
    DrawType=DT_Mesh
    bStatic=False
}

