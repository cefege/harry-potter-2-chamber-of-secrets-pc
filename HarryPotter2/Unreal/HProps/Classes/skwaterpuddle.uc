//===============================================================================
//  [skwaterpuddle] 
//===============================================================================

class skwaterpuddle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skwaterpuddleMesh MODELFILE=models\skwaterpuddle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skwaterpuddleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skwaterpuddleAnims ANIMFILE=models\skwaterpuddle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skwaterpuddleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skwaterpuddleMesh ANIM=skwaterpuddleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skwaterpuddleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skwaterpuddleTex0  FILE=TEXTURES\waterpuddleMap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skwaterpuddleMesh NUM=0 TEXTURE=skwaterpuddleTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: waterpuddleMap.bmp  Path: C:\HP2_master\chamber of secrets\psd 


defaultproperties
{
    Mesh=skwaterpuddleMesh
    DrawType=DT_Mesh
    bStatic=False
}

