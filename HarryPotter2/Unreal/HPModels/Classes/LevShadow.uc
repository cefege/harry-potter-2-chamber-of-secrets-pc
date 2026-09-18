//===============================================================================
//  [LevShadow] 
//===============================================================================

class LevShadow extends actor;
#exec MESH  MODELIMPORT MESH=LevShadowMesh MODELFILE=models\LevShadow.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=LevShadowMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=LevShadowAnims ANIMFILE=models\LevShadow.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=LevShadowMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=LevShadowMesh ANIM=LevShadowAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=LevShadowAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=LevShadowTex0  FILE=TEXTURES\Lev_Shad.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=LevShadowMesh NUM=0 TEXTURE=LevShadowTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: Lev_Shad.bmp  Path: D:\Harry Potter\Art\Objects\General Objects\Levitation Shadow 

defaultproperties
{
    Mesh=LevShadowMesh
    DrawType=DT_Mesh
    bStatic=False
}

