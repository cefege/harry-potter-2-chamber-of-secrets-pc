//=============================================================================
// SpawnThingy.
//=============================================================================
class SpawnThingy expands Triggers;

#exec Texture Import File=Textures\SthingE.pcx Name=SthingE Mips=Off Flags=2

var() class<Actor> SpawnClass;
var() float        fVelocityModifier;
var() name         SpawnTag;
var() bool         bThrowItem;
var() bool         bKeepingTryingSpawnZOnly;
var() float        fAdditionalZVelocity;

var(SpawnThingyPatrol) name         nameFirstPatrolPoint;
var(SpawnThingyPatrol) bool         bLoopPatrolPath;

function Trigger(Actor Other, Pawn Instigator)
{
	local	actor	SpawnedObject;

	local	vector	vel;
	local	rotator	SpawnDirection;

	if (SpawnClass != none)
	{
		if( SpawnTag != '' )
			SpawnedObject = FancySpawn(SpawnClass,,SpawnTag,Location,,bKeepingTryingSpawnZOnly);
		else
			SpawnedObject = FancySpawn(SpawnClass,,,Location,,bKeepingTryingSpawnZOnly);

		if ((nameFirstPatrolPoint != 'None') && SpawnedObject.isa('HPawn'))
		{
			HPawn(SpawnedObject).firstPatrolPointObjectName = nameFirstPatrolPoint;
			HPawn(SpawnedObject).bLoopPath = bLoopPatrolPath;
		}

		if(   bThrowItem
		   || SpawnedObject.isa('jellybean')  &&  bdirectional   	// only shoot out if the directional flag is set
		  )
		{
			// Move the bean out from this object
			vel.x = 96 - 32 + rand(64);
			vel.y = 0;
			vel.z = 40 + fAdditionalZVelocity;

			vel = vel >> rotation;
			SpawnedObject.Velocity = vel * fVelocityModifier;
			SpawnedObject.SetPhysics( PHYS_Falling );
			//SpawnedObject.GotoState('stateInfoPrint');
		}
	}
}

defaultproperties
{
     	
	Style=STY_Masked
     	Texture=SthingE
	fVelocityModifier=1.0
}
