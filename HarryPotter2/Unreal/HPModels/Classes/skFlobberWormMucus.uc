//===============================================================================
//  [skFlobberWormMucus] 
//===============================================================================

class skFlobberWormMucus extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skFlobberWormMucusMesh MODELFILE=models\skFlobberWormMucus.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlobberWormMucusMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skFlobberWormMucusMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlobberWormMucusMesh ANIM=skectoblobAnims

#EXEC TEXTURE IMPORT NAME=skFlobberWormMucusTex0  FILE=TEXTURES\FlobberwormMucus.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlobberWormMucusMesh NUM=0 TEXTURE=skFlobberWormMucusTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: FlobberwormMucus.bmp  Path: C:\potter\Characters\FlobberWorm 
