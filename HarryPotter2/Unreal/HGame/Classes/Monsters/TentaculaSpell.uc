// Class Name  : TentaculaSpell
//
// Created on  : 06/03/2002
// Authored by : Michael Lankerovich
// 
// Description : Tentacula AI. Can follow and attack Harry.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class TentaculaSpell extends GenericColObj;

auto state stateIdle
{
	function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
	{
		local TentaculaLimb		limb;
		local GenericColObj		hbox;

		local rotator rot;

		if(Owner.GetStateName() == 'stateStun')
			return false;
		if(Owner.GetStateName() == 'stateStunned')
			return false;
		if(Owner.GetStateName() == 'stateWake')
			return false;
		if(Owner.GetStateName() == 'stateDie')
		{
			eVulnerableToSpell = SPELL_none;
			return false;
		}
		if(Owner.GetStateName() == 'stateTwitch')
		{
			eVulnerableToSpell = SPELL_none;
			return false;
		}

		eVulnerableToSpell = SPELL_none;

		// set rotation, just in case
		rot.Yaw		= Owner.rotation.Yaw;
		rot.Roll	= 0;
		rot.Pitch	= 0;
		Owner.SetRotation(rot);

		Owner.GotoState('stateDie');

		// this limb is not good any more (will not be attached back in the future
		limb = TentaculaLimb(Owner);
		limb.bGood = false;

		// find head box of the limb with this Spell cylinder, and make it block players
		// so this limb could not damage Harry any more, but will not allow Harry to go throw.
		hbox = limb.headbox;
		if(hbox != none)
			hbox.SetCollision(true, true, true);

		// disconnect limb from the body
		limb.AnimBone = 0;
		limb.SetOwner( none );
 
		// disconnect spell from the limb
		AnimBone = 0;
		SetOwner( none );

		// destroy spell box
		Destroy();

		return true;
	}

	begin:
}

defaultproperties
{
	CollisionRadius=10
	CollisionHeight=10
	bRotateToDesired=false
	bHidden=true
	eVulnerableToSpell=SPELL_Diffindo;
}



 