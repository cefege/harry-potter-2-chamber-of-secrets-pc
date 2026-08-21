//===============================================================================
//  [skChristmasTree] 
//===============================================================================

class skChristmasTree extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChristmasTreeMesh MODELFILE=models\skChristmasTree.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChristmasTreeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChristmasTreeAnims ANIMFILE=models\skChristmasTree.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChristmasTreeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChristmasTreeMesh ANIM=skChristmasTreeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChristmasTreeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChristmasTreeTex0  FILE=TEXTURES\christree_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChristmasTreeMesh NUM=0 TEXTURE=skChristmasTreeTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: christree_256.bmp  Path: C:\Harry Potter\ART\Objects\Christmas Decorations\Tree 


defaultproperties
{
    Mesh=skChristmasTreeMesh
    DrawType=DT_Mesh
    bStatic=False
}

