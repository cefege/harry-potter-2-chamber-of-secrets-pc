//=============================================================================
// Eightball.
//=============================================================================
class Eightball extends Weapon;


// 3rd person perspective version
#exec MESH IMPORT MESH=8Ball3rd ANIVFILE=MODELS\8ball3_a.3D DATAFILE=MODELS\8ball3_d.3D X=0 Y=0 Z=0
#exec MESH ORIGIN MESH=8Ball3rd X=0 Y=-430 Z=-45 YAW=-64 ROLL=9
#exec MESH SEQUENCE MESH=8Ball3rd SEQ=All  STARTFRAME=0  NUMFRAMES=10
#exec MESH SEQUENCE MESH=8Ball3rd SEQ=Idle  STARTFRAME=0  NUMFRAMES=1
#exec MESH SEQUENCE MESH=8Ball3rd SEQ=Fire  STARTFRAME=1  NUMFRAMES=9
//#exec TEXTURE IMPORT NAME=JEightB1 FILE=MODELS\eightbal.PCX GROUP="Skins"
#exec MESHMAP SCALE MESHMAP=8Ball3rd X=0.065 Y=0.065 Z=0.13
//#exec MESHMAP SETTEXTURE MESHMAP=8Ball3rd NUM=1 TEXTURE=JEightB1
//#exec MESHMAP SETTEXTURE MESHMAP=8Ball3rd NUM=0 TEXTURE=UnrealShare.Effect18.FireEffect18




defaultproperties
{
}
