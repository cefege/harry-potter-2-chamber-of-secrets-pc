//===============================================================================
//  [skToolsMinersPick] 
//===============================================================================

class skToolsMinersPick extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skToolsMinersPickMesh MODELFILE=models\skToolsMinersPick.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skToolsMinersPickMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skToolsMinersPickAnims ANIMFILE=models\skToolsMinersPick.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skToolsMinersPickMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skToolsMinersPickMesh ANIM=skToolsMinersPickAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skToolsMinersPickAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skToolsMinersPickTex0  FILE=TEXTURES\MinersPick.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skToolsMinersPickMesh NUM=0 TEXTURE=skToolsMinersPickTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: MinersPick.bmp  Path: C:\Harry Potter\ART\Objects\Tools\Pick Axe 


defaultproperties
{
    Mesh=skToolsMinersPickMesh
    DrawType=DT_Mesh
    bStatic=False
}

