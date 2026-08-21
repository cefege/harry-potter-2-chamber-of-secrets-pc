//===============================================================================
//  [skSpellBall] 
//===============================================================================

class skSpellBall extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSpellBallMesh MODELFILE=models\skSpellBall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpellBallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpellBallAnims ANIMFILE=models\skSpellBall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpellBallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpellBallMesh ANIM=skSpellBallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpellBallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpellBallTex0  FILE=TEXTURES\stripebn_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpellBallMesh NUM=0 TEXTURE=skSpellBallTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: stripebn_64.bmp  Path: C:\HP2_Objects\SpellBall 


defaultproperties
{
    Mesh=skSpellBallMesh
    DrawType=DT_Mesh
    bStatic=False
}

