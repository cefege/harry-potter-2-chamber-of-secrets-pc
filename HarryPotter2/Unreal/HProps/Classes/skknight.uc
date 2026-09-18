//===============================================================================
//  [skKnight] 
//===============================================================================

class skKnight extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skKnightMesh MODELFILE=models\skknight.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skKnightMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skKnightAnims ANIMFILE=models\skknight.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skKnightMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skKnightMesh ANIM=skKnightAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skKnightAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skKnightTex0  FILE=TEXTURES\knight.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skKnightMesh NUM=0 TEXTURE=skKnightTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: knight.bmp  Path: C:\HP2_Objects\Armor\Suit of Armor 


defaultproperties
{
    Mesh=skKnightMesh
    DrawType=DT_Mesh
    bStatic=False
}

