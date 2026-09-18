//===============================================================================
//  [skCauldronSmall] 
//===============================================================================

class skCauldronSmall extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCauldronSmallMesh MODELFILE=models\skCauldronSmall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCauldronSmallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCauldronSmallAnims ANIMFILE=models\skCauldronSmall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCauldronSmallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCauldronSmallMesh ANIM=skCauldronSmallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCauldronSmallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCauldronSmallTex0  FILE=TEXTURES\couldron_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCauldronSmallMesh NUM=0 TEXTURE=skCauldronSmallTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: couldron_64.bmp  Path: C:\Harry Potter\ART\Objects\Cauldrons\Small Cauldron 


defaultproperties
{
    Mesh=skCauldronSmallMesh
    DrawType=DT_Mesh
    bStatic=False
}

