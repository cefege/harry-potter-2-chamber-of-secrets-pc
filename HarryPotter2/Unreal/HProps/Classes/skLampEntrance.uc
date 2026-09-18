//===============================================================================
//  [skLampEntrance] 
//===============================================================================

class skLampEntrance extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLampEntranceMesh MODELFILE=models\skLampEntrance.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLampEntranceMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLampEntranceAnims ANIMFILE=models\skLampEntrance.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLampEntranceMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLampEntranceMesh ANIM=skLampEntranceAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLampEntranceAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLampEntranceTex0  FILE=TEXTURES\lamppost_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLampEntranceMesh NUM=0 TEXTURE=skLampEntranceTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: lamppost_128.bmp  Path: C:\Harry Potter\ART\Objects\Lanterns_Lamps\Entrance Lamp Post 


defaultproperties
{
    Mesh=skLampEntranceMesh
    DrawType=DT_Mesh
    bStatic=False
}

