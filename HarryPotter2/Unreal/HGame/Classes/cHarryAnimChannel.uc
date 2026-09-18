
class cHarryAnimChannel expands AnimChannel;

var HProp propTemp;
var	int   LastAnimFrame;

//function GotoStatePickupItem()
//{
//	if( IsInState('stateIdle') )
//		GotoState('stateHoldUpArm');
//}

//function GotoStateIdle()
//{
//	GotoState('stateIdle');
//}

function GotoStateThrow()
{
	GotoState('stateThrow');
}

function bool GotoStateCasting( bool in_bHarryUsingSword )
{
	//if( in_bHarryUsingSword )
	//	GotoState( 'stateCast' );
	//else
		GotoState( 'stateCasting' );
}

//****************************************************************************
function bool IsCarryingActor()
{
	if(   IsInState( 'statePickupItem' )
	   || IsInState( 'stateThrow' )
	  )
		return true;
	else
		return false;
}

//****************************************************************************
function bool CanPickSomethingUp()
{
	return true;
}

//Normally true, you should just about always play harrys normal movement anims, unless you're doing
// an ecto jump for example, where you want the whole mesh to animate, but still be in PlayerWalking.
function bool PlayHarryMovementAnims()
{
	return true;
}

//Harry wont get the cast call, cause it now belongs to this AnimChannel, so pass it along.
function Cast()
{
	Harry(owner).Cast();
}

//*************************************************************
auto state stateIdle
{
	//NOTE: Gotta be carefull with stuff like this.  The PlayIdle call used to be done in Begin:  which means, someone
	//      could change this to stateIdle, play an anim on Harry, and then the next tick, this state code would override
	//      that anim call.  BAD!  So, better to use BeginState so it's done right away...
	function BeginState()
	{
		Harry(owner).PlayIdle();
	}

	//begin:
	//	Harry(owner).PlayIdle();
}

//*************************************************************************************
//*************************************************************************************
//*************************************************************************************
//*************************************************************************************

//*************************************************************
//pickup
//throwhold
//throw
state statePickupItem
{
	function bool CanPickSomethingUp()
	{
		return false;
	}

	function BeginState()
	{
		//Play the anim here instead of in Begin: so that it gets played when you call GotoState()
		PlayAnim( 'Pickup', 1, 0.15 );
	}

  begin:
	Harry(owner).clientmessage("hold up arm");

	Sleep( 0.2 );
	Harry(owner).AttachCarryActor();
	FinishAnim();
	//PlayAnim( 'trans2throw', 1.0 );
	LoopAnim( 'ThrowHold', 1, 0.1 );
}

state stateThrow
{
	function bool CanPickSomethingUp()
	{
		return false;
	}

  begin:
	PlayAnim( 'throw', 1.5 );
	sleep( 0.3375 / 1.5 );
	Harry(owner).ThrowCarryingActor();
	finishAnim();
	Harry(Owner).HarryAnimType = AT_Replace;
	GotoState('stateIdle');
}

//*************************************************************************************
//*************************************************************************************
//*************************************************************************************
//*************************************************************************************

//****************************************************************************
//function bool IsCasting()
//{
//	if(   IsInState( 'stateCasting' )
//	   || IsInState( 'stateCancelCasting' )
//	   || IsInState( 'stateCast' )
//	  )
//		return true;
//	else
//		return false;
//}

state stateCasting
{
	//function Tick(float dtime)
	//{
	//	if( Harry(Owner).GetStateName() != 'PlayerWalking' )
	//		GotoState('stateCancelCasting');
	//}
  begin:
	Harry(Owner).HarryAnimType = AT_Combine;
//	Harry(Owner).clientmessage("AnimType: " $Harry(Owner).HarryAnimType $" HarryAnimChannel: Casting");
	if( Harry(owner).bHarryUsingSword )
		LoopAnim( 'swordaim', 1.0, 0.2 );
	else
	{
		if(Harry(owner).bInDuelingMode )
			LoopAnim('duel_charge', 1.0, 0.2);
		else
			LoopAnim('castaim', 1.0, 0.2);
	}
}

state stateCancelCasting
{
  begin:
	Harry(Owner).clientmessage("HarryAnimChannel: CancelCasting");
	//Harry(owner).TurnOffCastingVars();

	Harry(owner).CurrIdleAnimName = Harry(owner).GetCurrIdleAnimName();
	PlayAnim(Harry(owner).CurrIdleAnimName, , 0.25);
	FinishAnim();

	Harry(Owner).StopAiming();
	//GotoState('stateIdle');	
}

state stateDuelingCast
{
/*
	function Tick(float dtime)
	{
		local int     Frame;
		super.Tick(dtime);

	 	Frame = AnimFrame * 33;

		if(	(Frame >= 20)  && (LastAnimFrame < 20) )
		{
			Harry(Owner).Cast();
		}

		LastAnimFrame = Frame;
	}
*/
  Begin:

	Harry(Owner).Cast();

	Harry(Owner).HarryAnimType = AT_Combine;

	PlayAnim('duel_cast', , [TweenTime]0.3);
	FinishAnim();

	// if button is not pressed, StopAiming
	if(Harry(Owner).bAltFire == 0)
		Harry(Owner).StopAiming();

	// otherwise, StartAiming
	else
	{
		Harry(Owner).TurnOffCastingVars();
		Harry(Owner).TurnOffSpellCursor();

		Harry(Owner).StartAiming(false);
	}
}

state stateDefenceCast
{
/*
	function Tick(float dtime)
	{
		local int     Frame;
		super.Tick(dtime);

	 	Frame = AnimFrame * 46;

		if(	(Frame >= 30)  && (LastAnimFrame < 30) )
		{
			Harry(Owner).Cast();
		}

		LastAnimFrame = Frame;
	}
*/
  Begin:

	// if was recently hit, do not rebounced
	if(Harry(Owner).fTimeAfterHit > 0)
		Harry(Owner).bReboundingSpells = false;
	else
		Harry(Owner).bReboundingSpells = true;

	Harry(Owner).Cast();

	Harry(Owner).HarryAnimType = AT_Combine;

	PlayAnim('cast_expelliarmus', , [TweenTime]0.3);
	FinishAnim();

	Harry(Owner).bReboundingSpells = false;

	// if button is not pressed, StopAiming
	if(Harry(Owner).bAltFire == 0)
		Harry(Owner).StopAiming();

	// otherwise, StartAiming
	else
	{
		Harry(Owner).TurnOffCastingVars();
		Harry(Owner).TurnOffSpellCursor();

		Harry(Owner).StartAiming(false);
	}
}

state stateCast
{
	function BeginState()
	{
		//DEBUG
//		Harry(owner).clientmessage("AnimType: " $Harry(Owner).HarryAnimType $" HarryAnimChannel: Cast.    bIsAiming:"$Harry(owner).bIsAiming);
		//Harry(owner).TurnOffCastingVars();  //Dont turn these off now, wait till the anim is done.

		if( Harry(owner).bHarryUsingSword )
			PlayAnim('SwordCast', 1.5, 0.1);
		else
  			PlayAnim('cast', 2.0, 0.1);
	}

  begin:
	FinishAnim();
	//Harry(owner).clientmessage("HarryAnimChannel: Cast 2.  bIsAiming:"$Harry(owner).bIsAiming);

	Harry(Owner).StopAiming();
	//GotoState('stateIdle');	
}

//*************************************************************************************
//*************************************************************************************
//*************************************************************************************
//*************************************************************************************
function DoKnockBack()
{
	if( !IsInState('stateKnockBack')   &&   !IsInState('stateEctoJump') )
		GotoState('stateKnockBack');
}

//********************
state stateKnockBack
{
	function bool CanPickSomethingUp()
	{
		return false;
	}

	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
//		Harry(Owner).ClientMessage("AnimType: " $Harry(Owner).HarryAnimType $" HarryAnimChannel: KnockBack");
	}

  Begin:

	//If harry's using the sword and he got hit, send off the charge, and cancel casting.
	if( Harry(Owner).bHarryUsingSword  &&  basewand(Harry(Owner).weapon).ChargingLevel() > 0 )
	{
		baseWand(Harry(Owner).weapon).CastSpell( Harry(Owner).weapon, , class'spellSwordFire' );
		Harry(Owner).StopAiming();
	}

	playanim('knockback', , 0.3); //[RootBone] 'move');
	FinishAnim();
	Harry(Owner).HarryAnimType = AT_Replace;

	if( Harry(Owner).PlayerIsAiming() )
	{
		//debug
//		Harry(Owner).ClientMessage(" Was aiming during the knockBack so we will go back to casting! ");

		GotoState( 'stateCasting' );
	}
	else
		GotoState('stateIdle');
}

//*************************************************************************
function DoEctoJump()
{
	if( !IsInState('stateEctoJump') )
		GotoState('stateEctoJump');
}

//********************
state stateEctoJump
{
	function bool CanPickSomethingUp()     {      return false; }
	function bool PlayHarryMovementAnims() {      return false; }

	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
	}

  Begin:
	playanim( Harry(Owner).HarryAnims[Harry(Owner).HarryAnimSet].Jump, , 0.1); //[RootBone] 'move');
	FinishAnim();
	Harry(Owner).HarryAnimType = AT_Replace;

	if( Harry(Owner).PlayerIsAiming() )
		GotoState( 'stateCasting' );
	else
		GotoState('stateIdle');
}


function DoDrinkWiggenwell()
{
	if (!IsInState('stateDrinkWiggenwell'))
		GotoState('stateDrinkWiggenwell');
}

state stateDrinkWiggenwell
{
	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
	}

	// When leave the state, make sure potion and status items get updated.
	function EndState()
	{
		// Get rid of the potion bottle
		propTemp.bHidden = true;
		Harry(Owner).DropCarryingActor();
		propTemp.Destroy();

		// Update the wiggenwell potions status item.
		Harry(Owner).managerStatus.GetStatusGroup(class'StatusGroupPotions').IncrementCount(class'StatusItemWiggenWell',-1);

		// Update health
		Harry(Owner).AddHealth(120);		
		PlaySound(sound'HPSounds.Magic_sfx.Health_boost1', SLOT_None);
	}

Begin:

   	// Put a potion bottle in Harry's left hand.
	propTemp = HProp(FancySpawn(class'WWellGreenBottle',Harry(Owner),,,Harry(Owner).Rotation));
	Harry(Owner).ActorToCarry = propTemp;
	Harry(Owner).AttachCarryActor('Bip01 L Forearm');	// LeftHand

    // Harry drinks animation
	PlayAnim('DrinkPotion',,[TweenTime]0.4);

	// Wait until Harry is actually drinking then play random gulp.
	sleep(0.5);
	switch( Rand(3) )
	{
		case 0:	PlaySound(sound'HPSounds.HAR_emotes.gulping2'); break;
		case 1:	PlaySound(sound'HPSounds.HAR_emotes.gulping3'); break;
		case 2:	PlaySound(sound'HPSounds.HAR_emotes.gulping4'); break;
		default:
			Harry(Owner).ClientMessage("Warning: random gulp not working right");
			PlaySound(sound'HPSounds.HAR_emotes.gulping2');
			break;
	}

	// Let Harry finish his anim
	FinishAnim();

	// Restore to normal
	Harry(Owner).HarryAnimType = AT_Replace;
	if( Harry(Owner).PlayerIsAiming() )
		GotoState( 'stateCasting' );
	else
		GotoState('stateIdle');
}


//*************************************************************************
function DoSleepyJump()
{
	if( !IsInState('stateSleepyJump') )
		GotoState('stateSleepyJump');
}

//********************
state stateSleepyJump
{
	function bool CanPickSomethingUp()     {      return false; }
	function bool PlayHarryMovementAnims() {      return false; }

	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
	}

  Begin:
	playanim( Harry(Owner).HarryAnims[Harry(Owner).HarryAnimSet].Jump, , 0.1); //[RootBone] 'move');
	FinishAnim();
	Harry(Owner).HarryAnimType = AT_Replace;

	if( Harry(Owner).PlayerIsAiming() )
		GotoState( 'stateCasting' );
	else
		GotoState('stateIdle');
}

//*************************************************************************
function DoWebJump()
{
	if( !IsInState('stateWebJump') )
		GotoState('stateWebJump');
}

//********************
state stateWebJump
{
	function bool CanPickSomethingUp()     {      return false; }
	function bool PlayHarryMovementAnims() {      return false; }

	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
	}

  Begin:
	playanim( Harry(Owner).HarryAnims[Harry(Owner).HarryAnimSet].Jump, , 0.1); //[RootBone] 'move');
	FinishAnim();
	Harry(Owner).HarryAnimType = AT_Replace;

	if( Harry(Owner).PlayerIsAiming() )
		GotoState( 'stateCasting' );
	else
		GotoState('stateIdle');
}
//*************************************************************************************
function DoReactRictusempra()
{
	if( !IsInState('stateReactRictusempra') )
		gotostate('stateReactRictusempra');
}

//********************
state stateReactRictusempra
{
	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
	}

  Begin:

	PlayAnim('react_rictusempra', , [TweenTime]0.3);
	FinishAnim();

	Harry(Owner).HarryAnimType = AT_Replace;

	if( Harry(Owner).PlayerIsAiming() )
		Harry(Owner).StopAiming();

	GotoState('stateIdle');
}
	
//*************************************************************************************
function DoReactMimbleWimble()
{
	if( !IsInState('stateReactMimbleWimble') )
		gotostate('stateReactMimbleWimble');
}

//********************
state stateReactMimbleWimble
{
	function BeginState()
	{
		Harry(Owner).HarryAnimType = AT_Combine;
	}

  Begin:

	PlayAnim('mimblewimble', , [TweenTime]0.3);
	FinishAnim();

	Harry(Owner).HarryAnimType = AT_Replace;

	if( Harry(Owner).PlayerIsAiming() )
		Harry(Owner).StopAiming();

	GotoState('stateIdle');
}
	