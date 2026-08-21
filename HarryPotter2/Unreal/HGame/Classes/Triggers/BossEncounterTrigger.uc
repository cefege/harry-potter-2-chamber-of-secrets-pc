
class BossEncounterTrigger extends trigger;

//var() bool   bSendTriggerOnTouch;
var() bool       bHarryShouldLockOntoBoss;
var() bool       bReverseInput;
var() bool       bKeepHarryFixed;
var() bool       bCanCast;
var() ESpellType ForceSpellType;
var() bool       bFixedFaceDirection;  //Uses direction this Trigger faces
var() bool       bDontNeedABoss;
var() bool       bExtendedTargetting;
var() bool       bSendTriggerToBoss;

var   bool       bDisabled;

//*******************************************************************************
//function TriggerToo( Actor Other, Pawn EventInstigator )
//{
//	Log("******** BossEncounterTrigger     TriggerFromHereToo");
//	DoTrigger(Other, EventInstigator);
//}

//*******************************************************************************
function Trigger( Actor Other, Pawn EventInstigator )
{
	DoTrigger( Other, EventInstigator );
}

//*******************************************************************************
function DoTrigger( Actor Other, Pawn EventInstigator )
{
	local baseBoss  boss;

Log("******** BossEncounterTrigger");

	if( !bDisabled )
	{
		ProcessTrigger();
		super.Touch( Other );

		if( bSendTriggerToBoss )
		{
			foreach AllActors( class'baseBoss', boss, Event )
			{
				Log("******** BossEncounterTrigger sent trigger '"$Event$"' to boss:"$boss);
				boss.Trigger(none, none);
			}
		}

		//boss.Trigger(none, none);	
	}
}

//*******************************************************************************
function Touch( actor Other )
{
	if( Harry(Other) == none )
		return;

	if( !bDisabled )
	{
		//if( bSendTriggerOnTouch )
			Super.Touch( Other );

		Log("******** boss encounter touch");

		ProcessTrigger();

		//if( bSendTriggerOnTouch )
		//	TriggerEvent( Event, none, none );
	}
}

//*******************************************************************************
function ProcessTrigger()
{
	local Harry h;
	local baseBoss  boss;
	local vector    vFixedFaceDirection;

	bDisabled = true;

	if( bFixedFaceDirection )
		vFixedFaceDirection = vector( Rotation );
	else
		vFixedFaceDirection = vect(0,0,0);

	//Find nearest boss
	foreach AllActors( class'baseBoss', boss, Event )
		break;

	Log("Found Boss:" $ boss);

	if( boss != none  ||  bDontNeedABoss )
	{
		foreach AllActors( class'Harry', h )
		{
			h.StartBossEncounter(boss, bHarryShouldLockOntoBoss, bReverseInput, bKeepHarryFixed, bCanCast, vFixedFaceDirection, ForceSpellType, bExtendedTargetting);
			break;
		}
	}
	else
	{
		Log("BossEncounterTrigger : Couldn't find boss to have encounter with");
	}

	//return boss;
}


//*****************************************************************************
defaultproperties
{
	bDirectional=true

	bHarryShouldLockOntoBoss=true
	bSendTriggerToBoss=true
	InitialState=none
}