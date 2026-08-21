
class spellTrigger extends trigger;

#exec Texture Import File=Textures\spell_trigger.pcx Name=spell_trigger Mips=Off Flags=2


function BeginPlay() 
{
	Super.BeginPlay();
}

//
// See whether the other actor is relevant to this trigger.
//
function bool IsRelevant( actor Other )
{
	if( !bInitiallyActive )
	{
		if(baseSpell(other)==None)
		{
			bInitiallyActive=true;
			log("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!making active1 "$other);
			return(false);
		}
		if( baseSpell(other).SpellType == eVulnerableToSpell )
			return(false);
		else
		{
			bInitiallyActive=true;
			log("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!making active 2 "$other);
			return(false);
		}
		return false;
	}

	//only spells trip this trigger.
	if(baseSpell(other)==None)
		return(false);

	if( baseSpell(other).SpellType == eVulnerableToSpell )
		return(true);
	else
		return(false);

}

function Touch( actor Other )
{
	local actor A;
	if( IsRelevant( Other ) )
	{
		if( bTriggerOnceOnly )
		{
			// Ignore future touches.
			SetCollision(False);
			bProjTarget = false;
		}
	}

	super.Touch(Other);
}

state() OtherTriggerToggles
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		Super.Trigger(Other, EventInstigator);
		bProjTarget = !bProjTarget;
	}
}

// Other trigger turns this on.
state() OtherTriggerTurnsOn
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		Super.Trigger(Other, EventInstigator);
		bProjTarget = true;
	}
}

// Other trigger turns this off.
state() OtherTriggerTurnsOff
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		Super.Trigger(Other, EventInstigator);
		bProjTarget = false;
	}
}

defaultproperties
{
     TriggerType=TT_Shoot
     bProjTarget=True
     Style=STY_Masked
     Texture=spell_trigger
}
