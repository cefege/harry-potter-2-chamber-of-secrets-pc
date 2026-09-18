//===============================================================================
//  SpellLessonWand used for spell lesson mechanic
//===============================================================================

class SpellLessonWand extends HProp;

var SpellLessonTrigger LessonTrigger;
var float              fWandSpeed;


function SetParentLessonTrigger(SpellLessonTrigger trigger)
{
	LessonTrigger = trigger;
}

function StartWand(float fSetSpeed)
{
    fWandSpeed = fSetSpeed;
    GoToState('PlayGame');
}

function StopWand()
{
    DestroyControllers();
	GoToState('Idle');
}

auto state Idle
{
}

state PlayGame
{
	function bool PawnAtInterpolationPoint( InterpolationPoint IPoint, 
		                                    InterpolationManager IManager )
	{
		return (LessonTrigger.WandAtInterpolationPoint(IPoint, IManager));
	}

	function BeginState()
	{
		KillAttachedParticleFx(0.0);
		CreateAttachedParticleFx();

		FollowSplinePath(LessonTrigger.nameSplinePath, fWandSpeed, 0, 
			             LessonTrigger.nameIPStart, LessonTrigger.nameIPEnd, false,
						 MOVE_TYPE_LINEAR, true);
	}

	function EndState()
	{
		// Stop ball sparks
		KillAttachedParticleFx(0.0);
	}

begin:
}

defaultproperties
{
	bAlignBottom=false
    Mesh=WandMesh
	DrawScale=1.5
    AmbientGlow=250
    CollisionRadius=10
    CollisionHeight=10
    bBlockActors=False
    bBlockPlayers=False
	bBlockCamera=false
	ePatrolType=PATROLTYPE_SPLINE_FOLLOW
	bIgnoreStationRotations=true
	attachedParticleClass(0)=Class'HPParticle.LessonSparks1'
    bRotateToDesired=false
}
