//===============================================================================
//  [skwallsconceCh] 
//===============================================================================

class skwallsconceCh extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skwallsconceChMesh MODELFILE=models\skwallsconceCh.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skwallsconceChMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skwallsconceChAnims ANIMFILE=models\skwallsconceCh.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skwallsconceChMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skwallsconceChMesh ANIM=skwallsconceChAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skwallsconceChAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skwallsconceChTex0  FILE=TEXTURES\wallsconceMap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skwallsconceChMesh NUM=0 TEXTURE=skwallsconceChTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: wallsconceMap.bmp  Path: C:\HP2_master\chamber of secrets\psd 


defaultproperties
{
    Mesh=skwallsconceChMesh
    DrawType=DT_Mesh
    bStatic=False
}

