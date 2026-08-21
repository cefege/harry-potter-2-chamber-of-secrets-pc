//===============================================================================
//  [skScrollBox] 
//===============================================================================

class skScrollBox extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skScrollBoxMesh MODELFILE=models\skScrollBox.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skScrollBoxMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skScrollBoxAnims ANIMFILE=models\skScrollBox.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skScrollBoxMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skScrollBoxMesh ANIM=skScrollBoxAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skScrollBoxAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skScrollBoxTex0  FILE=TEXTURES\DumbleScrollHolder.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skScrollBoxMesh NUM=0 TEXTURE=skScrollBoxTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: DumbleScrollHolder.bmp  Path: C:\HP2 Art\Textures 


defaultproperties
{
    Mesh=skScrollBoxMesh
    DrawType=DT_Mesh
    bStatic=False
}

