//===============================================================================
//  [skPadlock] 
//===============================================================================

class skPadlock extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPadlockMesh MODELFILE=models\skPadlock.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPadlockMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPadlockAnims ANIMFILE=models\skPadlock.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPadlockMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPadlockMesh ANIM=skPadlockAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPadlockAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPadlockTex0  FILE=TEXTURES\PadLock.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPadlockMesh NUM=0 TEXTURE=skPadlockTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: PadLock.bmp  Path: C:\Harry Potter\ART\Objects\skPadlock 


defaultproperties
{
    Mesh=skPadlockMesh
    DrawType=DT_Mesh
    bStatic=False
}

