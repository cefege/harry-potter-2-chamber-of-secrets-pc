//===============================================================================
//  [skStatueHunchbackWitch] 
//===============================================================================

class skStatueHunchbackWitch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skStatueHunchbackWitchMesh MODELFILE=models\skStatueHunchbackWitch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skStatueHunchbackWitchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skStatueHunchbackWitchAnims ANIMFILE=models\skStatueHunchbackWitch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skStatueHunchbackWitchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skStatueHunchbackWitchMesh ANIM=skStatueHunchbackWitchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skStatueHunchbackWitchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skStatueHunchbackWitchTex0  FILE=TEXTURES\Witch1.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skStatueHunchbackWitchTex1  FILE=TEXTURES\hatrimtwosided.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skStatueHunchbackWitchMesh NUM=0 TEXTURE=skStatueHunchbackWitchTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skStatueHunchbackWitchMesh NUM=1 TEXTURE=skStatueHunchbackWitchTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Witch1.bmp  Path: C:\Harry Potter\ART\Objects\Statues_Sculptures\Hunchback Witch 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: hatrimtwosided.bmp  Path: C:\Harry Potter\ART\Objects\Statues_Sculptures\Hunchback Witch 


defaultproperties
{
    Mesh=skStatueHunchbackWitchMesh
    DrawType=DT_Mesh
    bStatic=False
}

