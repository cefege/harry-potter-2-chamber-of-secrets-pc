//===============================================================================
//  [skChairsRedCouch] 
//===============================================================================

class skChairsRedCouch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsRedCouchMesh MODELFILE=models\skChairsRedCouch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsRedCouchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsRedCouchAnims ANIMFILE=models\skChairsRedCouch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsRedCouchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsRedCouchMesh ANIM=skChairsRedCouchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsRedCouchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsRedCouchTex0  FILE=TEXTURES\gryfsofa_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsRedCouchMesh NUM=0 TEXTURE=skChairsRedCouchTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: gryfsofa_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Red sofa 


defaultproperties
{
    Mesh=skChairsRedCouchMesh
    DrawType=DT_Mesh
    bStatic=False
}

