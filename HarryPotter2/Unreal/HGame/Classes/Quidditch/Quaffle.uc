//=============================================================================
// Quaffle  -- A red ball that can be held by players in Quidditch
//=============================================================================
class Quaffle extends QuidditchPawn;


//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
	if ( Mesh == None )
		Mesh = SkeletalMesh'HProps.skQuidditchQuaffleMesh';

	if ( ParticleTrail == None )
		ParticleTrail = class'Quaffle_FX';

	Super.PostBeginPlay();
}


defaultproperties
{
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HProps.skQuidditchQuaffleMesh'
	ParticleTrail=class'Quaffle_FX'
	IPSpeed=800;
    CollisionHeight=40.0
    CollisionRadius=40.0
}
