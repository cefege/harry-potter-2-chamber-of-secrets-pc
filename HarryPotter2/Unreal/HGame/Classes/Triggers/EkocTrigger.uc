class EkocTrigger expands Trigger;

function Activate( actor Other, pawn Instigator )
{
local string ekoc;

	ekoc=localize("[LevelEnable]","Enable","ABC.INT");
	if(instr(ekoc,"<")<0)
		super.activate(other,instigator);
}