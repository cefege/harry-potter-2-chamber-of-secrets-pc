//===============================================================================
//  [skJellybeanLowRes] 
//===============================================================================

class skJellybeanLowRes extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJellybeanLowResMesh MODELFILE=models\skJellybeanLowRes.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJellybeanLowResMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJellybeanLowResAnims ANIMFILE=models\skJellybeanLowRes.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJellybeanLowResMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJellybeanLowResMesh ANIM=skJellybeanLowResAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJellybeanLowResAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJellybeanLowResTex0  FILE=TEXTURES\jelybean_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJellybeanLowResMesh NUM=0 TEXTURE=skJellybeanLowResTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: jelybean_64.bmp  Path: C:\Harry Potter 2\ART\Objects\Food_Candy\Jellybeans 


defaultproperties
{
    Mesh=skJellybeanLowResMesh
    DrawType=DT_Mesh
    bStatic=False
}

