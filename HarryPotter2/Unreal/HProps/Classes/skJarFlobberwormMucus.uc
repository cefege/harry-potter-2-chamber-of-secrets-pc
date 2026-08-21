//===============================================================================
//  [skJarFlobberwormMucus] 
//===============================================================================

class skJarFlobberwormMucus extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarFlobberwormMucusMesh MODELFILE=models\skJarFlobberwormMucus.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarFlobberwormMucusMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarFlobberwormMucusAnims ANIMFILE=models\skJarFlobberwormMucus.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarFlobberwormMucusMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarFlobberwormMucusMesh ANIM=skJarFlobberwormMucusAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarFlobberwormMucusAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarFlobberwormMucusTex0  FILE=TEXTURES\FlobberwormMucus.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skJarFlobberwormMucusTex1  FILE=TEXTURES\FlobberJar.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skJarFlobberwormMucusTex2  FILE=TEXTURES\FlobberJar.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarFlobberwormMucusMesh NUM=0 TEXTURE=skJarFlobberwormMucusTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skJarFlobberwormMucusMesh NUM=1 TEXTURE=skJarFlobberwormMucusTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skJarFlobberwormMucusMesh NUM=2 TEXTURE=skJarFlobberwormMucusTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: FlobberwormMucus.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Folbberworm Mucus 
// Original material [1] is [SKIN01.TRANSLUCENT] SkinIndex: 1 Bitmap: FlobberJar.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Folbberworm Mucus 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: FlobberJar.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Folbberworm Mucus 


defaultproperties
{
    Mesh=skJarFlobberwormMucusMesh
    DrawType=DT_Mesh
    bStatic=False
}

