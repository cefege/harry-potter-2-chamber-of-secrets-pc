
class baseBasilisk expands baseBoss;

var() float HeadDamage;
var() float TailDamage;

//Overridden.   // This is now in HPawn
function ColObjTouch( actor other, GenericColObj ColObj )
{
}

defaultproperties
{
	Physics=PHYS_None
	HeadDamage=80
	TailDamage=3

    EnemyHealthBar=EnemyBar_Basilisk
}