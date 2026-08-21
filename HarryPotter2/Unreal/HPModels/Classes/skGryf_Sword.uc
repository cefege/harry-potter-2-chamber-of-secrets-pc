//===============================================================================
//  [skGryf_Sword] 
//===============================================================================

class skGryf_Sword extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGryf_SwordMesh MODELFILE=models\skGryf_Sword.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGryf_SwordMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skGryf_SwordAnims ANIMFILE=models\skGryf_Sword.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skGryf_SwordMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGryf_SwordMesh ANIM=skGryf_SwordAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skGryf_SwordAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skGryf_SwordTex0  FILE=TEXTURES\Gryf_Sword.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGryf_SwordMesh NUM=0 TEXTURE=skGryf_SwordTex0

// Original material [0] is [gryf_sword] SkinIndex: 0 Bitmap: Gryf_Sword.bmp  Path: C:\HP2_Objects\Gryfindor_Sword 
