//===============================================================================
//  [skgnome] 
//===============================================================================

class skgnome extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skgnomeMesh MODELFILE=models\skgnome.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skgnomeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skgnomeAnims ANIMFILE=models\skgnome.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skgnomeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skgnomeMesh ANIM=skgnomeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skgnomeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skgnomeTex0  FILE=TEXTURES\GNOME_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skgnomeTex1  FILE=TEXTURES\GNOME_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skgnomeTex2  FILE=TEXTURES\GNOME_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skgnomeMesh NUM=0 TEXTURE=skgnomeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skgnomeMesh NUM=1 TEXTURE=skgnomeTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skgnomeMesh NUM=2 TEXTURE=skgnomeTex2

#exec ANIM NOTIFY   ANIM=skgnomeAnims SEQ=runattack TIME=0.99 FUNCTION=PlayFootStep
#exec ANIM NOTIFY   ANIM=skgnomeAnims SEQ=runattack TIME=0.5 FUNCTION=PlayFootStep
#exec ANIM NOTIFY   ANIM=skgnomeAnims SEQ=runscared TIME=0.99 FUNCTION=PlayFootStep
#exec ANIM NOTIFY   ANIM=skgnomeAnims SEQ=runscared TIME=0.5 FUNCTION=PlayFootStep
#exec ANIM NOTIFY   ANIM=skgnomeAnims SEQ=runnormal TIME=0.99 FUNCTION=PlayFootStep
#exec ANIM NOTIFY   ANIM=skgnomeAnims SEQ=runnormal TIME=0.5 FUNCTION=PlayFootStep

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: GNOME_SKIN00.bmp  Path: H:\Art\Design\Creatures\Gnome 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: GNOME_SKIN01.bmp  Path: H:\Art\Design\Creatures\Gnome 
// Original material [2] is [SKIN02.MASKED] SkinIndex: 2 Bitmap: GNOME_SKIN02.bmp  Path: H:\Art\Design\Creatures\Gnome 
