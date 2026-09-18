// cAnimChannel allows us to play some animations (knockback, spell, ...) on top of a regular animations

class cAnimChannel expands AnimChannel;

var	int		LastAnimFrame;
var	bool	bCasting;

var float   fTimeInRictusempra;
var float   fTimeInMimbleWimble;

// Duellist wont get the cast call, cause it now belongs to this AnimChannel, so pass it along.
function Cast()
{
	Duellist(owner).Cast();
}

//*************************************************************
auto state stateIdle
{
	//NOTE: Gotta be carefull with stuff like this.  The PlayIdle call used to be done in Begin:  which means, someone
	//      could change this to stateIdle, play an anim on Harry, and then the next tick, this state code would override
	//      that anim call.  BAD!  So, better to use BeginState so it's done right away...
	function BeginState()
	{
		Duellist(owner).PlayIdle();
//		Duellist(owner).GotoState( Duellist(Owner).SavedState );
	}
}

//*************************************************************************************
function DoCast()
{
	if( !IsInState('stateCast') )
		gotostate('stateCast');
}

//********************
state stateCast
{
	function BeginState()
	{
		bCasting = true;
		Duellist(Owner).DuellistAnimType = AT_Combine;
	}

	function Tick(float dtime)
	{
		local int     Frame;
		super.Tick(dtime);

	 	Frame = AnimFrame * 33;

		if(	(Frame >= 20)  && (LastAnimFrame < 20) )
		{
			Duellist(Owner).Cast();
		}

		LastAnimFrame = Frame;
	}

  Begin:

	PlayAnim('cast', , [TweenTime]0.3);
	FinishAnim();

	Duellist(Owner).DuellistAnimType = AT_Replace;

	bCasting = false;

	GotoState('stateIdle');
}

//*************************************************************************************
function DoCharging()
{
	if( !IsInState('stateCharging') )
		gotostate('stateCharging');
}

//********************
state stateCharging
{
  Begin:

	Duellist(Owner).DuellistAnimType = AT_Combine;
	LoopAnim('duel_charge', , [TweenTime]0.3);

	GotoState('stateIdle');
}

//*************************************************************************************
function DoDefence()
{
	if( !IsInState('stateDefence') )
		gotostate('stateDefence');
}

//********************
state stateDefence
{
	function BeginState()
	{
		bCasting = true;
		Duellist(owner).bReboundingSpells = true;
		Duellist(Owner).DuellistAnimType = AT_Combine;
	}

/*
	function Tick(float dtime)
	{
		local int     Frame;
		super.Tick(dtime);

	 	Frame = AnimFrame * 46;

		if(	(Frame >= 30)  && (LastAnimFrame < 30) )
		{
			Duellist(Owner).Defence();
		}

		LastAnimFrame = Frame;
	}
*/
  Begin:

	Duellist(Owner).Defence();

 	PlayAnim('cast_expelliarmus', , [TweenTime]0.3);
 	FinishAnim();

	Duellist(Owner).DuellistAnimType = AT_Replace;

	Duellist(owner).bReboundingSpells = false;
	bCasting = false;

	GotoState('stateIdle');
}
													  
//*************************************************************************************
function DoKnockBack()
{
	if( !IsInState('stateKnockBack') )
		GotoState('stateKnockBack');
}

//********************
state stateKnockBack
{
  Begin:

	Duellist(Owner).DuellistAnimType = AT_Combine;

	PlayAnim('react_backfire', , [TweenTime]0.3);
	FinishAnim();

	Duellist(Owner).DuellistAnimType = AT_Replace;

	GotoState('stateIdle');
}

//*************************************************************************************
function DoReactRictusempra()
{
	if( !IsInState('stateReactRictusempra') )
		gotostate('stateReactRictusempra');
}

//********************
function QuitThisState(bool rictusempra)
{
	Duellist(Owner).gotoState('statePatrol');

	if(rictusempra)
		Duellist(Owner).StartCharging();

	GotoState('stateIdle');
}

state stateReactRictusempra
{
	function Tick(float dtime)
	{
		fTimeInRictusempra += dtime;

		// if it is dying playing animation, quit after 2 seconds
		if( fTimeInRictusempra > 2)
			QuitThisState(true);
	}

	function BeginState()
	{
		Duellist(Owner).DuellistAnimType = AT_Combine;
		fTimeInRictusempra = 0;
	}

  Begin:

	Duellist(Owner).StopCharging();

	PlayAnim('react_rictusempra', , [TweenTime]0.3);
	Sleep(0.1);		// just in case, if animation was not played
	FinishAnim();

	Duellist(Owner).DuellistAnimType = AT_Replace;

	QuitThisState(true);
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
	function Tick(float dtime)
	{
		fTimeInMimbleWimble += dtime;

		// if it is dying playing animation, quit after 2 seconds
		if( fTimeInMimbleWimble > 2)
			QuitThisState(false);
	}

	function BeginState()
	{
		Duellist(Owner).DuellistAnimType = AT_Combine;
		fTimeInMimbleWimble = 0;
	}

  Begin:

  	Duellist(Owner).StopCharging();

	PlayAnim('mimblewimble', , [TweenTime]0.3);
	Sleep(0.1);		// just in case, if animation was not played
	FinishAnim();

	Duellist(Owner).DuellistAnimType = AT_Replace;

	QuitThisState(false);
}

