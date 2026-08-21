//===============================================================================
//  [skDragonSkull] 
//===============================================================================

class skDragonSkull extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDragonSkullMesh MODELFILE=models\skDragonSkull.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDragonSkullMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDragonSkullAnims ANIMFILE=models\skDragonSkull.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDragonSkullMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDragonSkullMesh ANIM=skDragonSkullAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDragonSkullAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDragonSkullTex0  FILE=TEXTURES\drgskull_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDragonSkullMesh NUM=0 TEXTURE=skDragonSkullTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: drgskull_128.bmp  Path: C:\Harry Potter\ART\Objects\Dragon Skull 


defaultproperties
{
    Mesh=skDragonSkullMesh
    DrawType=DT_Mesh
    bStatic=False
}

