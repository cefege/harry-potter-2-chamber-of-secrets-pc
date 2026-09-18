class Duellist extends Characters;

const MAX_DUEL_COMMENT_NAMES = 10;
const MAX_DUEL_COMMENT_HOUSE_NAMES = 6;
const MAX_DUEL_COMMENT_VARIANTS = 10;			// If this changes, change Variants[] in struct VarCommentInfo too

const NUM_HURT_SOUNDS = 15;

///////////////////////////////////////////////////////////////////////////////////////////////
// Duel Comments Variables 
///////////////////////////////////////////////////////////////////////////////////////////////

enum DuelComment	// If changed, update size of Comments[] array, and defaultproperties below
{
	DC_None,				//0	
	DC_DuelIntro,			//1	
	DC_DuelLose,			//2	
	DC_DuelWin,				//3	
	DC_DuelHry,				//4	
	DC_DuelOpp,				//5	
	DC_NumComments			//6 
};

var string DuelCommentNames[10];	//MAX_DUEL_COMMENT_NAMES

enum HouseAffiliation	// Must be in same order as in Duellist.uc
{
	HA_Gryffindor,		// House 0 (also used for generic house)
	HA_Ravenclaw,		// House 1 (also used for generic opponent)
	HA_Hufflepuff,		// House 2
	HA_Slytherin,		// House 3

	HA_Neutral,			// Used when the specific house isn't important
};

var string DuelCommentHouseNames[6];//MAX_DUEL_COMMENT_HOUSE_NAMES

struct CommentInfo {
	var String			DlgName;			// Name used to lookup dialog assets for comment
	var Sound			DlgSound;			// Sound asset for comment
	var String			DlgText;			// Display text for comment
	var bool			bHasBeenSaid;		// Has this comment already been said before
	var float			fTimeLastSaid;		// When was it last said
};

struct VarCommentInfo {
	var CommentInfo		Variant[10]; 		// All interchangable variations of this comment
	var int				Variations;			// How many variations of this comment are there
	var bool			bHasBeenSaid;		// Has any variant of this comment already been said before
	var float			fTimeLastSaid;		// When was any variant last said
};

struct HouseDependentCommentInfo {
	var VarCommentInfo	House;				// House-specific variations of this comment
	var bool			bHasBeenSaid;		// Has any house variant of this comment already been said before
	var float			fTimeLastSaid;		// When was any variant last said
};

var HouseDependentCommentInfo Comments[9];			// Info on all the comments the duelling commentator can make

var float				fNextTimeACommentCanBeSaid;	// When is it safe to say another comment
var float				fGapTime;					// How much silence to put between comments

const fNoGapTimeBetweenComments  = 0.1;		// Sliver of time between end of last comment and beginning of next when no gap is desired
const fMinGapTimeBetweenComments = 1.0;		// Lowest Minimum time required between end of last comment and beginning of next
const fMaxGapTimeBetweenComments = 2.4;		// Highest Minimum time required between end of last comment and beginning of next

const fMinTimeBeforeCommentRepeat = 30.0;	// How much time must elapse before a certain comment can be said again

var ProfSnape	pSnape;

///////////////////////////////////////////////////////////////////////////////////////////////
// Duellist Variables
///////////////////////////////////////////////////////////////////////////////////////////////

var EAnimType	DuellistAnimType;

var cAnimChannel   DuellistAnimChannel;

var SpellCursor	SpellCursor;

var vector		vTargetDir;
var vector		vNewLoc;

var int		    nMaxHealth;
var	float		ftemp;
var	float		fTimeAfterHit;

var	float		fLimitInXDirection;

var	vector		StartLocation;
var	vector		HarryStartLocation;

var() name		WonEventName;
var() name		LostEventName;

var() float		Intellect;
var() bool		bGoToDuelMode;

var() HouseAffiliation	eHouse;

var bool		bReboundingSpells;

var class<baseSpell> CurrentSpellClass;

function bool HarrySpellAboutToHitMe()
{
	local int i;
	local baseSpell	CurrCastedSpell;

	// could not defence yourself right after hit for awhile
	if(fTimeAfterHit > 0)
		return false;

	for( i = 0; i < BaseWand(playerHarry.weapon).NumCastedSpells;  i++)
	{
		CurrCastedSpell = BaseWand(playerHarry.weapon).CastedSpellList[i];

		if( vsize(CurrCastedSpell.Location - Location) < CollisionRadius * (2 + Intellect) )
			return true;
	}

	return false;
}

function bool HarrySpellGoesInMyDirection()
{
	local int i;
	local float y, t;
	local vector loc, vel;
	local baseSpell	CurrCastedSpell;

	// could not defence yourself right after hit for awhile
	if(fTimeAfterHit > 0)
		return false;

	// I am already charging Expelliarmus spell
	if( CurrentSpellClass == class'spellDuelExpelliarmus' ) 
		return false;

	for( i = 0; i < BaseWand(playerHarry.weapon).NumCastedSpells;  i++)
	{
		CurrCastedSpell = BaseWand(playerHarry.weapon).CastedSpellList[i];

		loc = CurrCastedSpell.Location;
		vel = CurrCastedSpell.Velocity;

		if(vel.X == 0)
			continue;

		t = (location.X - loc.X) / vel.X;

		if(t < 0)
			continue;

		y = loc.Y + vel.Y * t;

		if(abs(y - location.Y) < collisionRadius * (1 + Intellect * 0.5) )
			return true;
	}

	return false;
}

function StartCharging()
{
	if(	playerHarry.bDuelIsOver )
		return;

	// do not charge, if recently was hit
	if(fTimeAfterHit > 0)
		return;

	// if Harry was hit recently, use just Rictusempra to damage him more
	if(playerHarry.fTimeAfterHit > 0)
		CurrentSpellClass = class'spellDuelRictusempra';
	else
	{
		switch( Rand(2) )
		{
			case 0: 
				CurrentSpellClass = class'spellDuelRictusempra';
				break;
			case 1: 
				CurrentSpellClass = class'spellDuelMimblewimble';
				break;
			default:
				CurrentSpellClass = class'spellDuelRictusempra';
				break;
		}
	}

	baseWand(weapon).StartChargingSpell( true, false, CurrentSpellClass );

	DuellistAnimChannel.DoCharging();
}

function StartChargingDefence()
{
	CurrentSpellClass = class'spellDuelExpelliarmus';

	baseWand(weapon).StartChargingSpell( true, false, CurrentSpellClass );

	DuellistAnimChannel.DoCharging();
}

function StopCharging()
{
	baseWand(weapon).StopChargingSpell();
}

function TurnOffSpellCursor()
{
	SpellCursor.TurnTargetingOff();
	baseWand(weapon).StopChargingSpell();
}

function Cast()
{
	baseWand(weapon).CastSpell( playerHarry, , CurrentSpellClass);
	
	// Turn off our SpellCursor
	TurnOffSpellCursor();
}

function Defence()
{
 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_EXP_swoosh' );
	baseWand(weapon).CastSpell( playerHarry, , class'spellDuelExpelliarmus' );

	// Turn off our SpellCursor
	TurnOffSpellCursor();
}

//-------------------------------------------------------------------------------------------
// Dialog functions
//-------------------------------------------------------------------------------------------

function float TimeLeftUntilSafeToSayAComment( optional bool bNoGap )
{
	local float	fTimeLeft;

	fTimeLeft = fNextTimeACommentCanBeSaid - Level.TimeSeconds + 0.1;
	if ( bNoGap )
		fTimeLeft += fNoGapTimeBetweenComments;
	else
		fTimeLeft += fGapTime;

	return fTimeLeft;
}

function bool CommentHasBeenSaidBefore( DuelComment eComment )
{
	// Returns true if the any variant of the indicated comment has ever been
	// said before.

	return Comments[ eComment ].bHasBeenSaid;
}

function bool SayComment( DuelComment eComment, optional HouseAffiliation eHouse, optional bool bNoGap )
{
	// Makes the duel commentator say the indicated comment. 
	// Will automatically choose among available interchangable variants.
	// May not say the comment if it has been said too recently or the last one (plus a gap) hasn't finished yet.  
	// If bNoGap is true, then the silence gap after last comment doesn't have to be finished.
	// Returns True if comment was actually said.

	local bool				bSaid;

	local int				Variant;
	local int				Tied;
	local int				OldestVariant;
	local float				OldestTime, duration;

	local Sound				DlgSound;

	bSaid = false;
	if ( eComment == DC_None )
		return false;

	// play Harry and Opponent's commentary, not very often
	if ( (eComment == DC_DuelHry) || (eComment == DC_DuelOpp) )
	{
		if(Rand(2) == 0)
			return false;
	}

	// Skip comment if too soon to say another one
	if ( bNoGap )
	{
		if ( Level.TimeSeconds < fNextTimeACommentCanBeSaid + fNoGapTimeBetweenComments )
			return false;
	}
	else
	{
		if ( Level.TimeSeconds < fNextTimeACommentCanBeSaid + fGapTime )
			return false;
	}

	// Pick a variant (find oldest one; randomize on ties)
	OldestVariant = 0;
	OldestTime = Level.TimeSeconds;
	Tied = 0;
	Variant = 0;
	while (    Variant < MAX_DUEL_COMMENT_VARIANTS
		    && Comments[ eComment ].House.Variant[ Variant ].DlgName != "" )
	{
		if ( Comments[ eComment ].House.Variant[ Variant ].bHasBeenSaid )
		{
			if ( Comments[ eComment ].House.Variant[ Variant ].fTimeLastSaid < OldestTime )
			{
				OldestVariant = Variant;
				OldestTime = Comments[ eComment ].House.Variant[ Variant ].fTimeLastSaid;
			}
		}
		else
		{
			// Never been said before; randomly break tie among others not said before
			++Tied;
			if ( FRand() <= 1.0/Tied )	// Iteratively gives even weight across all choices so far
			{
				OldestVariant = Variant;
				OldestTime = -1.0;
			}
		}

		++Variant;
	}

	Comments[ eComment ].House.Variations = Variant;	// Remember count
	Variant = OldestVariant;

log("...................eComment = " $eComment $" eHouse = " $eHouse $" Variant = " $Variant);

	// Say the comment (if not too soon to repeat it)
	if (    !Comments[ eComment ].House.Variant[ Variant ].bHasBeenSaid
		 || ( Level.TimeSeconds > Comments[ eComment ].House.Variant[ Variant ].fTimeLastSaid + fMinTimeBeforeCommentRepeat ) )
	{
		DlgSound = Comments[ eComment ].House.Variant[ Variant ].DlgSound;

log("...................Sound = " $DlgSound $" eComment = " $eComment $" eHouse = " $eHouse $" Variant = " $Variant);

		// Mark when this comment was said
		Comments[ eComment ].House.Variant[ Variant ].fTimeLastSaid = Level.TimeSeconds;
		Comments[ eComment ].House.fTimeLastSaid = Level.TimeSeconds;
		Comments[ eComment ].fTimeLastSaid = Level.TimeSeconds;

		Comments[ eComment ].House.Variant[ Variant ].bHasBeenSaid = true;
		Comments[ eComment ].House.bHasBeenSaid = true;
		Comments[ eComment ].bHasBeenSaid = true;

		fGapTime = FRand() * (fMaxGapTimeBetweenComments-fMinGapTimeBetweenComments) + fMinGapTimeBetweenComments;

		if ( DlgSound != None )
		{
			if(pSnape != none)
				pSnape.PlaySound( DlgSound, SLOT_Talk, , , 10000.0 );	// Radius makes sure commentator can be heard far away
			else
				PlaySound( DlgSound, SLOT_Talk, , , 10000.0 );			// Radius makes sure commentator can be heard far away

			bSaid = true;

			// Figure out when its safe to say another comment
			fNextTimeACommentCanBeSaid = Level.TimeSeconds + GetSoundDuration( DlgSound );
		}
		else
		{
			duration = DeliverLocalizedDialog(Comments[ eComment ].House.Variant[ Variant ].DlgName, true, 0.0);

			bSaid = false;

			// Figure out when its safe to say another comment
			fNextTimeACommentCanBeSaid = Level.TimeSeconds + duration;

//			Log( "DuelCommentator: Failed to say dialog for type "$eComment$" comment; DlgName = "
//				 $Comments[ eComment ].House.Variant[ Variant ].DlgName$"." );
		}
	}
	else
	{
//		Log( "DuelCommentator: Skipping dialog for type "$eComment$" comment, variant "$Variant$"; DlgName = "
//			 $Comments[ eComment ].House.Variant[ Variant ].DlgName$"." );
	}

	return bSaid;
}

function string EventNumToEventName(int num)
{
	return(DuelCommentNames[num]);
}

function string GetCommentId(int eventNum,int house,int variant)
{
	local string key;
	local string eventName;
	local string id;

	eventName=EventNumToEventName(eventNum);

	key=DuelCommentHouseNames[house]$"_"$eventName;

	id=Localize( key,"line"$variant,"DuelSet" );

	//check to see if line actually exists.
	if(instr(id,"<")>-1)
		return("");	//doesnt exist
	else
		return(id);

}

function fillCommentArray()
{
	local int c,h,v;
	local string sndId;

	local DuelComment vvv;

	h = eHouse;

	for(c = 0; c < MAX_DUEL_COMMENT_NAMES; c++)
	{
		for(v = 0; v < MAX_DUEL_COMMENT_VARIANTS; v++)
		{
			sndId=GetCommentId(c,h,v);

			if(sndId!="")
			{
				Comments[c].House.Variant[v].DlgName=sndId;
				Comments[c].House.Variant[v].DlgSound=Sound( DynamicLoadObject("AllDialog."$sndId, class'Sound') );
			}
		}
	}
}

//*********************************************************************************************

function PostBeginPlay()
{
	local weapon weap;
	local string animName;
	local int	i;
	local name	nm;

	Super.PostBeginPlay();

	if(Intellect < 0)
		Intellect = 0;

	if(Intellect > 1)
		Intellect = 1;

	weap = spawn(class'baseWand', [SpawnOwner]self);
	weap.BecomeItem();
	weap.WeaponSet(self);
	weap.GiveAmmo(self);

	DuellistAnimChannel = cAnimChannel( CreateAnimChannel(class'cAnimChannel', AT_Replace, 'bip01 spine1') );
	DuellistAnimChannel.SetOwner( self );

	SpellCursor = spawn(class'SpellCursor');

	if(bGoToDuelMode)
		playerHarry.TurnOnDuelingMode(self);

	fNextTimeACommentCanBeSaid = 0.0;
	fGapTime = 0.0;

	//load sound names from DuelSet.int
	fillCommentArray();

	foreach AllActors( class'ProfSnape', pSnape )
		break;

	// to be sure, designer did not reset it (very important!)
	bRotateToDesired = default.bRotateToDesired;
}

function SetHealthBar()
{
	local  EnemyHealthManager   EHealth;
	EHealth = EnemyHealthManager( FancySpawn(class'EnemyHealthManager') );
	EHealth.Start( self);
}

function int DeltaHealth(bool HarryHealth, int spellType, float SpellCharge)
{
	local float fDelta, charge, MaxHealthRict, MaxHealthMimb, MaxHealth;
	local int   iDelta;

	local float MaxDamageRictusempra, MaxDamageMimblewimble;

	MaxHealthRict = nMaxHealth;
	MaxHealthMimb = nMaxHealth / 5.0;

	// if Harry was hit by weak spell, make it a bit stronger
	charge = SpellCharge;
	if(HarryHealth && (charge < 1))
		charge = 1;

	if(spellType == 0)				// Rictusempra
		MaxHealth = MaxHealthRict;
	else if(spellType == 1)			// Mimblewimble
		MaxHealth = MaxHealthMimb;
	else							// just in case
		MaxHealth = MaxHealthMimb;

	// give Harry more damage, if duellist intellect is higher
	if(HarryHealth)
		fDelta = MaxHealth / 4;
	else
		fDelta = (0.25 * Intellect * Intellect - 0.625 * Intellect + 0.5) * MaxHealth;

	fDelta *= charge;

	if(fDelta < 1)	
		fDelta = 1;

	if(fDelta > nMaxHealth / 2.0)	
		fDelta = nMaxHealth / 2.0;

	// just in case
	if(SpellCharge == 0)
		fDelta = 0;

	iDelta = fDelta;

//	if(HarryHealth)
//		log("Harry: " $"Type = " $spellType $" iDelta = " $iDelta $" fDelta = " $fDelta $" SpellCharge = " $SpellCharge);
//	else
//		log("Duell: " $"Type = " $spellType $" iDelta = " $iDelta $" fDelta = " $fDelta $" SpellCharge = " $SpellCharge);

	return iDelta;
}

//******************************************************************************************
function float GetHealth()
{
	return float(Health) / nMaxHealth;
}

//*********************************************************************************************
function PlayIdle()
{
	LoopAnim( 'duel_idle', 0.8, [TweenTime]0.25, , DuellistAnimType );
}

//*********************************************************************************************
// --- Handle Dueling spell functions

function bool HandleSpellDuelExpelliarmus( optional baseSpell spell, optional vector vHitLocation )
{
	return false; // *in-valid* hit
}

//*********************************************************************************************
function HarryWonDuel()
{
	TurnOffSpellCursor();

	playerHarry.bDuelIsOver = true;

	playerHarry.UpdateDuelingRanks(true);

	SentEvent(WonEventName);

	gotoState('stateIdle');
	DuellistAnimChannel.gotoState('stateIdle');
}

//******************************************************************************************
function bool HandleSpellDuelRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	local float SpellCharge, SpellSpeed;
	
	// See if we are currently rebounding spells
	if( bReboundingSpells )
	{		
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_EXP_smack', Slot_Misc );

		// send this spell back at the caster
		spell.Reflect( self, FMin( 5,    spell.SpellCharge + ( 5 - spell.SpellCharge ) * 0.10f ), 
							 FMin( 1000, spell.Speed       + ( 1000 - spell.Speed  )   * 0.25f ));
		baseWand(weapon).FlashChargeParticles( class'Exep_Shield' );
		return false;
	}

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	PlayIdle();	// just in case (to fix some problems in cAnimChannel)

	Health -= DeltaHealth(false, 0, spell.SpellCharge);

 	PlaySound( playerHarry.HurtSound[ Rand(NUM_HURT_SOUNDS) ] );

	if(Health > 0)
	{
		SayComment( DC_DuelHry, eHouse, true );
		DuellistAnimChannel.DoReactRictusempra();
	}
	else
	{
		SayComment( DC_DuelWin, eHouse, true );
		HarryWonDuel();
	}

	fTimeAfterHit = 1.0;

	return true;
}

function bool HandleSpellDuelMimblewimble( optional baseSpell spell, optional vector vHitLocation )
{
	// See if we are currently rebounding spells
	if( bReboundingSpells )
	{		
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_EXP_smack', Slot_Misc );

		// send this spell back at the caster
		spell.Reflect( self, FMin( 5,    spell.SpellCharge + ( 5 - spell.SpellCharge ) * 0.10f ), 
							 FMin( 1000, spell.Speed       + ( 1000 - spell.Speed  )   * 0.25f ));
		baseWand(weapon).FlashChargeParticles( class'Exep_Shield' );
		return false;
	}

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	PlayIdle();	// just in case (to fix some problems in cAnimChannel)

	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_MIM_hit', Slot_Misc);

	if(Rand(2) == 0)
	{
		Health -= DeltaHealth(false, 1, spell.SpellCharge);
		PlaySound( Sound'HPSounds.Magic_sfx.Dueling_MIM_self_damage', Slot_Misc);
	}
	else
		PlaySound( Sound'HPSounds.Magic_sfx.Dueling_MIM_self_lucky', Slot_Misc);

	if(Health > 0)
	{
		SayComment( DC_DuelHry, eHouse, true );
		DuellistAnimChannel.DoReactMimblewimble();
	}
	else
	{
		SayComment( DC_DuelWin, eHouse, true );
		HarryWonDuel();
	}

	fTimeAfterHit = 6.0 - Intellect;

	return true;
}

function bool CouldTauntHarry()
{
	if( playerHarry.fTimeAfterHit < 1 + 4 * Intellect )
		return false;

	if( IsInState('stateTaunt') )
		return false;

	if( !DuellistAnimChannel.IsInState('stateIdle') )
		return false;

	return true;
}

//*********************************************************************************************
// overwrite stateIdle (need to use bGoToDuelMode)
state stateIdle
{
  Begin:

	if(bGoToDuelMode)
	{
		playerHarry.TurnOnDuelingMode(self);
		gotostate('stateStartDuel');
	}

	PlayAnim( 'idle' );
	FinishAnim();
	Sleep(0.001);	  // just in case

	Goto 'Begin';
}

state stateShot
{
	function BeginState()
	{
		DuellistAnimChannel.bCasting = true;
	}

	function Tick(float dtime)
	{
		super.Tick(dtime);
		if(DuellistAnimChannel.bCasting)
			return;

		// if shooting is over, go to patrolling
		StartCharging();
		gotostate('statePatrol');
	}

	begin:

		// Stop moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);

		if(AnimSequence != 'duel_idle' )
		{
			Sleep(0.001);
			AnimRate = 0;

			LoopAnim('duel_idle');
		}

		DuellistAnimChannel.DoCast();
}

state stateDefence
{
	function BeginState()
	{
		DuellistAnimChannel.bCasting = true;
	}

	function Tick(float dtime)
	{
		super.Tick(dtime);
		if(DuellistAnimChannel.bCasting)
			return;

		StartCharging();
		gotostate('statePatrol');
	}

	begin:

		// Stop moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);

		if(AnimSequence != 'duel_idle' )
		{
			Sleep(0.001);
			AnimRate = 0;

			LoopAnim('duel_idle');
		}

		DuellistAnimChannel.DoDefence();
}

state stateStay		// do not move
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	PlayIdle();
	Sleep(0.05);
	
	// start patroling
	gotostate('statePatrol');
}

state stateGoRight		// negative Y axes
{
	function HitWall(vector HitNormal, actor HitWall)
	{
		vNewLoc.Y	= location.Y;
		vNewLoc.Z	= location.Z;
		vNewLoc.X   = location.X + 10;

		// if hit the side wall, go backward a little bit
		gotostate('stateGoBackward');
	}

	begin:

 	// if it was a different anim. sequence,
	// give it a few miliseconds to stop previous animation and start the new one
	if(AnimSequence != 'strafe_right' )
	{
		Sleep(0.001);
		AnimRate = 0;

		LoopAnim('strafe_right');
	}

	MoveTo(vNewLoc);
	
	// start patroling
	gotostate('statePatrol');
}

state stateGoLeft		// positive Y axes
{
	function HitWall(vector HitNormal, actor HitWall)
	{
		vNewLoc.Y	= location.Y;
		vNewLoc.Z	= location.Z;
		vNewLoc.X   = location.X + 10;

		// if hit the side wall, go backward a little bit
		gotostate('stateGoBackward');
	}

	begin:

 	// if it was a different anim. sequence, 
	// give it a few miliseconds to stop previous animation and start the new one
	if(AnimSequence != 'strafe_left' )
	{
		Sleep(0.001);
		AnimRate = 0;

		LoopAnim('strafe_left');
	}

	MoveTo(vNewLoc);
	
	// start patroling
	gotostate('statePatrol');
}

state stateGoForward		// negative X axes
{
	begin:

	// give it a few miliseconds to stop previous animation
	Sleep(0.001);
	AnimRate = 0;

	LoopAnim('duel_run');

	MoveTo(vNewLoc);
	
	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// start patroling
	gotostate('statePatrol');
}

state stateGoBackward		// positive X axes
{
	begin:

	// give it a few miliseconds to stop previous animation
	Sleep(0.001);
	AnimRate = 0;

	LoopAnim('duel_runback');

	MoveTo(vNewLoc);
	
	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// start patroling
	gotostate('statePatrol');
}

state stateFollowHarry_In_Y_Dir
{
	begin:

	vNewLoc		= playerHarry.location;
	vNewLoc.X	= location.X;
	vNewLoc.Z	= location.Z;

	if(	vNewLoc.Y > location.Y )
		gotostate('stateGoLeft');
	else if( vNewLoc.Y < location.Y )
		gotostate('stateGoRight');
	
	// just in case
	gotostate('statePatrol');
}

state stateFollowHarry_In_X_Dir
{
	begin:

	vNewLoc.Y	= location.Y;
	vNewLoc.Z	= location.Z;

	if( location.X >= StartLocation.X )
	{
		vNewLoc.X = location.X - RandRange(fLimitInXDirection / 2, fLimitInXDirection);
		gotostate('stateGoForward');
	}
	else
	{
		vNewLoc.X = location.X + RandRange(fLimitInXDirection / 2, fLimitInXDirection);
		gotostate('stateGoBackWard');
	}
	
	// just in case
	gotostate('statePatrol');
}

state stateRunFromHarry
{
	begin:

	// if far from Harry, just stay and wait
	if(	abs(playerHarry.location.Y - location.Y) > 25 * (1 + Intellect) )
		gotostate('stateStay');

	vNewLoc	= location;

	if( playerHarry.location.Y > HarryStartLocation.Y)				// if Harry is on a left side of arena
		vNewLoc.Y = playerHarry.location.Y - 25 * (1 + Intellect);	// move to the right a little bit
	else															// else if Harry is on a right side of arena
		vNewLoc.Y = playerHarry.location.Y + 25 * (1 + Intellect);	// move to the left a little bit

	if(	vNewLoc.Y > location.Y )
		gotostate('stateGoLeft');
	else if( vNewLoc.Y < location.Y )
		gotostate('stateGoRight');
	
	// just in case
	gotostate('statePatrol');
}

state statePatrol
{
	begin:

	// if was hit recently, run away
	if(fTimeAfterHit > 0)
		gotostate('stateRunFromHarry');

	ftemp = playerHarry.location.Y - location.Y;
	if( abs(ftemp) < playerHarry.CollisionRadius )
	{
		if( CouldTauntHarry() )
		{
			gotostate('stateTaunt');
		}

		else if( LineOfSightTo(playerHarry) && (CurrentSpellClass != class'spellDuelExpelliarmus') && ( baseWand(weapon).ChargingLevel() > 0.25 * (2 - Intellect) ) && (fTimeAfterHit <= 0) )
		{
			gotostate('stateShot');
		}

		else
		{
			if(	Rand(5) == 0)
			{
				gotostate('stateFollowHarry_In_X_Dir');
			}
			else
				gotostate('stateStay');
		}
	}
	else
	{
		gotostate('stateFollowHarry_In_Y_Dir');
	}
}

state stateTaunt
{
  Begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	TurnOffSpellCursor();

	// give it a few miliseconds to stop previous animation
	Sleep(0.001);
	AnimRate = 0;

	if(Rand(2) == 0)
		PlayAnim('taunt_1');
	else
		PlayAnim('taunt_2');

	FinishAnim();

	StartCharging();

	gotostate('statePatrol');
}

state stateStartDuel
{
  Begin:

	StartLocation		= location;
	HarryStartLocation	= playerHarry.location;

	SayComment( DC_DuelIntro, eHouse, true );

	PlayAnim('duel_idle');
	FinishAnim();

	StartCharging();

	gotostate('statePatrol');
}

state stateDead
{
	//Use this, cause it gets called when you call GotoState();
	function BeginState()
	{
		Velocity	= vect(0,0,0);
		Acceleration= vect(0,0,0);
	}

  begin:

	Level.Game.RestartGame();
}

function Tick(float deltaT)
{
	local float savedTimeAfterHit;

	super.Tick(deltaT);

//log("DState = " $GetStateName() $" AState = " $DuellistAnimChannel.GetStateName() $" DAnim = " $AnimSequence $" AAnim = " $DuellistAnimChannel.AnimSequence $" AFrame = " $AnimFrame $" X = " $location.X $" Y = " $location.Y $" Z = " $location.Z);

	savedTimeAfterHit = fTimeAfterHit;
	if(	fTimeAfterHit > 0)
		fTimeAfterHit -= deltaT;

	// do not charge while after hitting , then do it once
	if( (savedTimeAfterHit > 0) && (fTimeAfterHit <= 0) )
		StartCharging();

	// if Duellist is behind the 'wall', his wand position is not updates (engine stuff)
	// and charging spell is sitting at last visisble position, which is wrong
	// So, stop him, as soon as he is not visible
	if( playerHarry.bInDuelingMode && !LineOfSightTo(playerHarry) ) 
	{
		if( !IsInState('stateStay') )
			gotostate('stateStay');
	}

	// if started to defend himself, but spell missed, he will hold Expelliarmus forever.
	// I need to rest it somehow, do it if defence spell is at full charge
	if( (CurrentSpellClass == class'spellDuelExpelliarmus') && (baseWand(weapon).ChargingLevel() >= 1) )
	{
		TurnOffSpellCursor();
		StartCharging();
	}

	if( HarrySpellGoesInMyDirection() )
	{
			StartChargingDefence();
	}

	if( (CurrentSpellClass == class'spellDuelExpelliarmus') && (baseWand(weapon).ChargingLevel() > 0.2 * (1 - Intellect)) && HarrySpellAboutToHitMe() )
	{
		gotostate('stateDefence');
	}
}

function SentEvent(name EName)
{
	if(EName != 'none')
	{
		TriggerEvent(EName, none, none );

		playerHarry.TurnOffDuelingMode();

		playerHarry.clientMessage("Send event.................." $EName);
	}
}

defaultproperties
{
	DrawType=DT_Mesh
//	Mesh=SkeletalMesh'HPModels.skHermioneMesh'	// Mesh=SkeletalMesh'HPModels.skHP2_GenFeMale1Mesh'	//	Mesh=SkeletalMesh'HPModels.skRonMesh'
	Mesh=SkeletalMesh'HPModels.skHP2_GenFeMale1Mesh'
	AmbientGlow=65

	CollisionRadius=15
	CollisionHeight=44

	SightRadius=512

	eVulnerableToSpell=SPELL_none

	bRotateToDesired=false

	Intellect=1.0

	bGoToDuelMode=false

	WonEventName=HarryWonDuel
	LostEventName=HarryLostDuel

	fLimitInXDirection=100

//	eHouse=HA_Gryffindor
	eHouse=HA_Slytherin

	DuelCommentHouseNames(0)="WCG";		// 0	
	DuelCommentHouseNames(1)="WCH";		// 1	
	DuelCommentHouseNames(2)="WCR";		// 2	
	DuelCommentHouseNames(3)="WCS";		// 3	
	DuelCommentHouseNames(4)="WCN";		// 4	
	DuelCommentHouseNames(5)="WCO";		// 5	

	DuelCommentNames(0)="None"			// 0
	DuelCommentNames(1)="DuelIntro"		// 1
	DuelCommentNames(2)="DuelLose" 		// 2
	DuelCommentNames(3)="DuelWin" 		// 3
	DuelCommentNames(4)="DuelHry"		// 4
	DuelCommentNames(5)="DuelOpp"		// 5

    EnemyHealthBar=EnemyBar_Duellist
}