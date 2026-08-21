//===============================================================================
//  [skDragonSkeleton] 
//===============================================================================

class skDragonSkeleton extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDragonSkeletonMesh MODELFILE=models\skDragonSkeleton.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDragonSkeletonMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDragonSkeletonAnims ANIMFILE=models\skDragonSkeleton.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDragonSkeletonMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDragonSkeletonMesh ANIM=skDragonSkeletonAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDragonSkeletonAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDragonSkeletonTex0  FILE=TEXTURES\DragWingTailFeets.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDragonSkeletonTex1  FILE=TEXTURES\DragWingTailFeets.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDragonSkeletonTex2  FILE=TEXTURES\DragBackLegs.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDragonSkeletonTex3  FILE=TEXTURES\drgskull_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDragonSkeletonMesh NUM=0 TEXTURE=skDragonSkeletonTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skDragonSkeletonMesh NUM=1 TEXTURE=skDragonSkeletonTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skDragonSkeletonMesh NUM=2 TEXTURE=skDragonSkeletonTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skDragonSkeletonMesh NUM=3 TEXTURE=skDragonSkeletonTex3

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: DragWingTailFeets.bmp  Path: C:\Harry Potter 2\ART\Objects\Dragon Skull\Dragon Skeleton 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: DragWingTailFeets.bmp  Path: C:\Harry Potter 2\ART\Objects\Dragon Skull\Dragon Skeleton 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: DragBackLegs.bmp  Path: C:\Harry Potter 2\ART\Objects\Dragon Skull\Dragon Skeleton 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: drgskull_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Dragon Skull 


defaultproperties
{
    Mesh=skDragonSkeletonMesh
    DrawType=DT_Mesh
    bStatic=False
}

