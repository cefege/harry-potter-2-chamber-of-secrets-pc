//===============================================================================
//  [skRedBlackStripeBean] 
//===============================================================================

class skRedBlackStripeBean extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skRedBlackStripeBeanMesh MODELFILE=models\skRedBlackStripeBean.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skRedBlackStripeBeanMesh X=0 Y=0 Z=16 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skRedBlackStripeBeanAnims ANIMFILE=models\skRedBlackStripeBean.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skRedBlackStripeBeanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skRedBlackStripeBeanMesh ANIM=skRedBlackStripeBeanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skRedBlackStripeBeanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skRedBlackStripeBeanTex0  FILE=TEXTURES\stripebn_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skRedBlackStripeBeanMesh NUM=0 TEXTURE=skRedBlackStripeBeanTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: stripebn_64.bmp  Path: D:\Harry Potter\A Lorian's Stuff\Hogwarts\General Objects\Candy 


defaultproperties
{
    Mesh=skRedBlackStripeBeanMesh
    DrawType=DT_Mesh
    bStatic=False
}

