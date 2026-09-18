class BossRailMove expands baseBoss;


var sound   peevesVoice;
var vector vMoveDir;
var vector vMoveTo;
var float   MoveSpeed;

//Make two NavigationPoint actors that define where the 'boss' moves back and forth between.  This is
// the 'tag' that the two nav points have in their event catagory
var() name NavPoint_MoveTagName;

//This is another NavigationPoint actor that defines how far away harry can back up during an encounter.
var() name NavPoint_HarryDistanceTagName;

//These are refs to the two rails that you move between
var NavigationPoint RailEnd1, RailEnd2;
var NavigationPoint HarryDistNavPoint;

var vector          vClosestPointOnRail;  //closest point on rail to HarryDistNavPoint
var vector          vNormalToHarryMoveRail;
var float            DistToHarryMoveRail;

var bool            bMovingLeft;

//      ^
//      | n1
//    1---------2
//    |         |
//    |         |-->
//    |         | n2
//    |         |
//    3---------4
//       
//       
//
//  1 and 2 are RailEnd1 and RailEnd2, or RailEnd2 and RailEnd1 (doesn't matter which)

var vector             v1,v2,v3,v4;
var vector             n1,n2;

//var vector          vCollAct1, vCollAct2, vCollAct3, vCollAct4;

var int                ProjectileCount;
var int                MoveCount;

var bool               bDoDirectionChange;

var int                ThrowNInARow;
var bool               bHaveThrownYet;
var() name             TriggerToSendOnFirstThrow;

var   bool             bDidFirstMove;

var() class<baseSpell> SpellToCast;

var() name             StartAnim;

var   bool             bDoTakeDamageSpeech;

var   float            PlayHarryHurtTimer;
var   float            PlayNoSoundsTimer;

var   int              iCurrentOuchSfx;
var   int              iHitSfx;
var   int              iRandomSfx;
var   int              iMalfoyHitHarrySfx;

var   vector           vPosSave;
var   bool             bFlyBackToVPosSave;

var   sound            TempSound;
var   float            TempTime;
var   rotator          TempRotator;

//*************************************************************************************************************************
function PostBeginPlay()
{
	local baseWand weap;
	local NavigationPoint A;
	local vector v;
	local vector v0;
	local actor act;

	Super.PostBeginPlay();

	//weap=spawn(class'baseWand');
	//weap.BecomeItem();
	//AddInventory(weap);
	//weap.WeaponSet(self);
	//weap.GiveAmmo(self);
	//weap.bAutoSelectSpell = false;
	//weap.SetCurrentSpell(SpellToCast);
	//SetPhysics(PHYS_Walking);

	//SetCollision( true, true, true );
	//SetCollisionSize( CollisionRadius*0.75*DrawScale, 20 );//float NewRadius, float NewHeight );

	vMoveDir.x = 1;
	vMoveDir.y = 0;
	vMoveDir.z = 0;

	MoveSpeed = GroundSpeed;

	//Find our two rail points.  They'll be tagged with NavPoint_MoveTagName.
	foreach AllActors( class 'NavigationPoint', A, NavPoint_MoveTagName )
	{
		if( RailEnd1 == none )
		{
			RailEnd1 = A;
		}
		else
		{
			RailEnd2 = A;
			break;
		}
	}

	foreach AllActors( class 'NavigationPoint', A, NavPoint_HarryDistanceTagName )
	{
		HarryDistNavPoint = A;
		break;
	}

	if( RailEnd1 != none  &&  RailEnd2 != none  &&  HarryDistNavPoint != none )
	{
		//I'm sure this is all horribly unneccesary...

		//Get a normal between the two rail ends
		v0 = normal( RailEnd2.Location - RailEnd1.Location );

		//Set v to be the vector from RailEnd1, along the rail, to the perpendicular line from the rail to HarryDistNavPoint
		v = v0  *  ((HarryDistNavPoint.Location - RailEnd1.Location) dot v0);

		//Now just set vClosestPointOnRail to that actual location
		vClosestPointOnRail = RailEnd1.Location + v;

		//Subtract to get vNormalToHarryMoveRail (not normalized)
		vNormalToHarryMoveRail = HarryDistNavPoint.Location - vClosestPointOnRail;     //(HarryDistNavPoint.Location - RailEnd1.Location) - vClosestPointOnRail;

		//Get that distance
		DistToHarryMoveRail = VSize(vNormalToHarryMoveRail);

		//Normalize vNormalToHarryMoveRail
		vNormalToHarryMoveRail = normal(vNormalToHarryMoveRail);



		//build box around the fight area.  probably should be done in the editor.  This really sucks...
		v1 = RailEnd1.Location + (vNormalToHarryMoveRail/* - v0*/) * 120;
		v2 = RailEnd2.Location + (vNormalToHarryMoveRail/* + v0*/) * 120;
		v3 = v1 + vNormalToHarryMoveRail * (DistToHarryMoveRail - 80);
		v4 = v2 + vNormalToHarryMoveRail * (DistToHarryMoveRail - 80);

		n1 = normal(v1 - v3);
		n2 = normal(v2 - v1);
		
		/*
		vCollAct1 = v1 + (v3-v1)/2 - v0                    *5000;
		vCollAct2 = v2 + (v1-v2)/2 - vNormalToHarryMoveRail*5000;
		vCollAct3 = v3 + (v4-v3)/2 + vNormalToHarryMoveRail*5000;
		vCollAct4 = v4 + (v2-v4)/2 + v0                    *5000;

		act = spawn( class'skbarrel', , 'BossCollFightTag', vCollAct1 );
		act.SetCollisionSize( 5000, 80 );
		//act.SetCollision( true, false, true );
		act = spawn( class'skbarrel', , 'BossCollFightTag', vCollAct2 );
		act.SetCollisionSize( 5000, 80 );
		//act.SetCollision( true, false, true );
		act = spawn( class'skbarrel', , 'BossCollFightTag', vCollAct3 );
		act.SetCollisionSize( 5000, 80 );
		//act.SetCollision( true, false, true );
		act = spawn( class'skbarrel', , 'BossCollFightTag', vCollAct4 );
		act.SetCollisionSize( 5000, 80 );
		//act.SetCollision( true, false, true );
		*/
	}

	FindNewMoveToLoc();

	SetTimer( 0.2, true );

	//loopanim(StartAnim);
}

//*************************************************************************************************************************
function Timer()
{
	if( PlayHarryHurtTimer > 0 )
	{
		PlayHarryHurtTimer -= 0.2;

		if( PlayHarryHurtTimer <= 0 )
		{
			PlayHarryHurtTimer = 0;
			PlayMalfoyHitHarrySfx();

			PlayNoSoundsTimer = 2.0;  //dont play any more speech for a bit
		}
	}


	if( PlayNoSoundsTimer > 0 )
	{
		PlayNoSoundsTimer -= 0.2;

		if( PlayNoSoundsTimer < 0 )
			PlayNoSoundsTimer = 0;
	}
}

//*************************************************************************************************************************
function float GetHealth()
{
	return float(InumHitstoBeat-INumHits) /float(iNumHitsToBeat);
}

//*************************************************************************************************************************
function Trigger( actor Other, pawn EventInstigator )
{
	local actor act;

	GotoState('PatrolForHarry');
	playerharry.clientmessage(SpellToCast);

	PlayDialogLoud( "PC_PVS_happy01fx" );

	//foreach allactors(class'actor', act, 'BossCollFightTag')
	//	act.SetCollision( true, false, true );
}

//*************************************************************************************************************************
function rotator AdjustAim(float projSpeed, vector projStart, int aimerror, bool bLeadTarget, bool bWarnTarget)
{
	local rotator r;
	local float   degrees, d;

	r = rotator( playerHarry.Location - Location );
	
	//This will need to be fixed later...

	//  map 0 to DistToHarryMoveRail   to   5 to 45 degrees
	degrees = FClamp( VSize2d(playerHarry.Location - Location), 0, DistToHarryMoveRail ) / DistToHarryMoveRail; //This maps it from 0 to 1
	degrees = degrees*degrees * 40.0   +   5.0;  //squaring degrees makes it work better

	//d = FClamp( VSize2d(playerHarry.Location - Location), 5, DistToHarryMoveRail ) / DistToHarryMoveRail;
	//d *= 0.707; //sqrt( 0.5 ); //to 45 degrees
	//d = sqrt( 1.0/(d*d) - 1.0 ); //convert d to y/x
	//degrees = atan( d ) * 360 / 2 / Pi;

	degrees = 90.0 - degrees;

	r.pitch = degrees * 65536.0 / 360.0;
	return r;
}

//*************************************************************************************************************************
function bool HandleSpellSkurge( optional baseSpell spell, optional vector vHitLocation )
{
	TakeDamage( 5, none, vect(0,0,0), vect(0,0,0), '' );
	return true;
}

//*************************************************************************************************************************
function bool FlyBackToVPosSave() { return false; }
function bool RespondToDamage()   { return true;  }

//*************************************************************************************************************************
event TakeDamage( int Damage, Pawn instigatedBy, Vector hitlocation, Vector momentum, name damageType)
{
	//super.takedamage(Damage, instigatedBy,hitlocation, momentum, damageType);
	playerHarry.clientmessage("boss takes damage");

	bFlyBackToVPosSave = FlyBackToVPosSave();

	PlayOuchSfx(false);

	iNumHits++;

	if( iNumHits >= iNumHitsToBeat )
	{
		//PlayAnim('prefly', 1.0, 0.3);
		//FinishAnim();
		//PlayAnim('idle', 1.0, 0.3);
		//Sleep(1);
		//loopAnim(StartAnim);
		
		//Start cut scene...
		SendDefeatedTrigger();

		//GotoState('stateIdle');
		playerHarry.StopBossEncounter();

		CleanupAfterBoss();

		SetPhysics( PHYS_Flying );

		GroundSpeed = 300;

		GotoState('stateIdle');
	}
	else // a normal hit
	{
		if( RespondToDamage() )
		{
			PlayAnim('prefly', 1.0, 0.3);

			if( bFlyBackToVPosSave )
				GotoState( 'stateHit2' );
			else
				GotoState( 'stateHit3' );
		}
	}

}

//*************************************************************************************************************************
function PlayRunAnim()
{
	if( bMovingLeft )
		loopAnim('Flying', 1, 1);
	else
		loopAnim('Flying', 1, 1);
}

//*************************************************************************************************************************
function FindNewMoveToLoc()
{
	local int i;
	local float d;

	if( RailEnd1==none || RailEnd2==none )
	{
		vMoveTo = Location + VRand();
		playerHarry.clientMessage("No rail ends for boss");
	}
	else
	{
		if( !bDidFirstMove )
		{
			bDidFirstMove = true;
			vMoveTo = (RailEnd2.Location + RailEnd1.Location) / 2;
		}
		else
		{
			d = VSize2D(RailEnd2.Location - RailEnd1.Location);

			//Try a few times to find a good random loc
			for( i = 0; i < 5; i++)
			{
				vMoveTo = RailEnd1.Location + (RailEnd2.Location - RailEnd1.Location)*FRand();

				if( VSize2D(vMoveTo - Location) > d/5 )
					break;
			}
		}

		if( ((RailEnd2.Location - RailEnd1.Location) dot (vMoveTo - Location)) < 0 )
			bMovingLeft = true;
		else	
			bMovingLeft = false;
	}

	vMoveDir = Normal( vMoveTo - Location );
}

//*************************************************************************************************************************
auto state stateIdle
{
	function bool RespondToDamage()   { return false;  }
  Begin:
	LoopAnim(StartAnim);
}

//*************************************************************************************************************************
state PatrolForHarry
{
	//* * * * * * * * * * * * * * * * * * * * * * *
	function BeginState()
	{
		FindNewMoveToLoc();
	}

	//* * * * * * * * * * * * * * * * * * * * * * *
	function touch(actor other)
	{
		//	PlaySound(sound 'filch', SLOT_Interact, 3.2, false, 2000.0, 1.0);
		//	playerHarry.clientMessage(self $":" $other $" touched me!");
		//playerHarry.clientMessage(self $":touch");
		//gotostate('attackHarry');
		//playerHarry.clientMessage(self $":touch");

		//if( Harry(other) != None )
		//	GotoState('DoFlip');

		if( Harry(other) != none )
		{
			//Harry(other).vAdditionalAccel = vMoveDir * 400;
			Harry(other).vAdditionalAccel = ( (RailEnd2.Location - RailEnd1.Location) cross vect(0,0,1) ) * 400;

		}
	}

	//* * * * * * * * * * * * * * * * * * * * * * *
	function bump(actor other)
	{
		touch( other );
	}

	//* * * * * * * * * * * * * * * * * * * * * * *
	function HitWall(vector HitNormal, actor Wall)
	{
		SetLocation( OldLocation );//+ (HitNormal * 50));
		//Find a new vMoveDir

		//vMoveDir = -vMoveDir;

		//gotostate('TurnToNewDir');
	}

	//* * * * * * * * * * * * * * * * * * * * * * *
	function Tick(float DeltaTime)
	{
		local vector   newloc;
		local actor    a;
		local float    fMoveDist;
		local float    d;
		local rotator  r;
		//local int      i;

		fMoveDist = MoveSpeed * DeltaTime;

		vMoveDir = vMoveTo - Location;
		vMoveDir.z = 0;
		d = VSize( vMoveDir );
		vMoveDir = normal(vMoveDir);
		if( d < 50 )
			vMoveDir *= (d+50)/100;
		MoveSmooth( vMoveDir * fMoveDist);
		r = rotator(playerHarry.Location - Location);
		r.pitch = 0;
		//SetRotation( r );
		DesiredRotation = r;

		//See if we got to vMoveTo
		if( VSize2d(Location - vMoveTo) < fMoveDist )
		{
			//Sometimes, move to a new location again
			if( bDoDirectionChange && FRand() < 0.25 )
			{
				bDoDirectionChange = false;
				GotoState('ShortWait');
			}
			else
			{
				//Toss out a few in a row, always now
				// need an audio queue
				//if( FRand() < 0.075 )
				//	ThrowNInARow = 3;
				//else
				switch( playerHarry.Difficulty )
				{
					case DifficultyEasy:    ThrowNInARow = 2;   break;
					case DifficultyMedium:  ThrowNInARow = 4;   break;
					case DifficultyHard:    ThrowNInARow = 6;   break;
				}
				//ThrowNInARow = 1;//Rand( 0.0 + (1.0 - GetHealth()) * 5 );

				bDoDirectionChange = true;
				GotoState('attackHarry');
			}
		}
	}

  Begin:
	AirSpeed=500;
	PlayRunAnim();
}

//*************************************************************************************************************************
state ShortWait
{
  begin:
	//loopAnim('breath');
	PlayAnim('prefly', 2, 0.25);
	PlayDialogLoud( "PC_PVS_happy01fx", [Pitch]RandRange(0.8, 1.2) );

	DesiredRotation = Rotation;

	//PlayRandomSfx();

	//sleep(0.75);
	finishAnim();
	PlayRunAnim();
	GotoState('PatrolForHarry');
}

//*************************************************************************************************************************
/*state TurnToNewDir
{
  begin:
	loopAnim('breath');

  wait:
	sleep( 0.5 );
		
	TurnTo( location + vMoveDir );
	gotostate('PatrolForHarry');
	goto 'wait';

}
*/
//	PlaySound(sound 'own_good', SLOT_Interact, 3.2, false, 2000.0, 1.0);
//	PlaySound(sound 'please', SLOT_Interact, 3.2, false, 2000.0, 1.0);
//	PlaySound(sound 'own_good', SLOT_Interact, 3.2, false, 2000.0, 1.0);
//	PlaySound(sound 'student', SLOT_Interact, 3.2, false, 2000.0, 1.0);

//*************************************************************************************************************************
state attackHarry
{
  begin:

	if( FRand() < 0.3 )
		GotoState( 'stateTaunt1' );

	PlayAnim('prefly', 2.0, 0.2);

	if( !bHaveThrownYet )
	{
		bHaveThrownYet = true;
		TriggerEvent( TriggerToSendOnFirstThrow, self, self );
	}

  wait:
	DesiredRotation.yaw = rotator( playerHarry.Location - Location ).yaw;
	FinishAnim();
	DesiredRotation.yaw = rotator( playerHarry.Location - Location ).yaw;
	gotostate('throwing');

}

//*************************************************************************************************************************
state stateTaunt1
{
	function bool FlyBackToVPosSave() { return true; }

	function BeginState()
	{
		vPosSave = Location;

		DoFlyTo_Actor( playerHarry, vect(30,0,80), MOVE_TYPE_EASE_FROM_AND_TO, 0.65, /*bFixedToChar*/false, /*bStayLockedToActor*/true );

		switch( rand(3) )
		{
			case 0:   PlayAnim( 'Taunt',   1.0, 0.5 );   break;   //66 frames
			case 1:   PlayAnim( 'Taunt_2', 1.0, 0.5 );   break;   //91
			case 2:   PlayAnim( 'Taunt_3', 1.0, 0.5 );   break;   //66
		}

		switch( rand(7) )
		{
			case 0:    PlayDialogLoud( "PC_PVS_happy02fx", [Pitch]RandRange(0.8, 1.2) );   break;
			case 1:    PlayDialogLoud( "PC_PVS_happy03fx", [Pitch]RandRange(0.8, 1.2) );   break;
			case 2:    PlayDialogLoud( "PC_PVS_happy04fx", [Pitch]RandRange(0.8, 1.2) );   break;
			case 3:    PlayDialogLoud( "PC_PVS_happy05fx", [Pitch]RandRange(0.8, 1.2) );   break;
			case 4:    PlayDialogLoud( "PC_PVS_happy06fx", [Pitch]RandRange(0.8, 1.2) );   break;
			case 5:    PlayDialogLoud( "PC_PVS_Chal2Skurge_51", [Pitch]RandRange(0.8, 1.2) );   break;
			case 6:    PlayDialogLoud( "PC_PVS_Chal2Skurge_53", [Pitch]RandRange(0.8, 1.2) );   break;
		}
	}

	function EndState()
	{
		DestroyControllers();
	}

	function OnEvent( name EventName )
	{
		if( EventName == 'ActionDone' );
			GotoState( 'stateTaunt2' );
	}
}

//*************************************************************************************************************************
state stateTaunt2
{
	function bool FlyBackToVPosSave() { return true; }

	function EndState()
	{
		DestroyControllers();
	}

	function OnEvent( name EventName )
	{
		if( EventName == 'ActionDone' )
			gotostate('PatrolForHarry');
	}

  Begin:
	Sleep( 0.5 );
	DoFlyTo( vPosSave, MOVE_TYPE_EASE_FROM_AND_TO, 0.65 );
}

//*************************************************************************************************************************
state throwing
{
	begin:
		DesiredRotation.yaw = rotator( playerHarry.Location - Location ).yaw;

		PlayAnim('throw', 1.0, 0.2);

		sleep( 17.0/30.0 );
		DesiredRotation.yaw = rotator( playerHarry.Location - Location ).yaw;

		//target = playerHarry;
		ShootSpell();

		if( FRand() < 0.5 )
			TempSound = sound'HPSounds.Critters_sfx.peeves_throw';
		else
			TempSound = sound'HPSounds.Critters_sfx.peeves_throw2';
		PlaySound( TempSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );

		//baseWand(weapon).CastSpell( playerHarry );
		//baseWand(weapon).LastCastedSpell.target = none;  //no autotargetting
		//baseWand(waapon).LastCastedSpell.

		//Every nth projectile goes off quick!
		ProjectileCount++;
		if( ProjectileCount >= 3 )
		{
			//			// commented out because AdjustLifeTimer is obsolete in baseSpell.uc
			//			baseWand(weapon).LastCastedSpell.AdjustLifeTimer(2.75);
			ProjectileCount = 0;
		}

		if( ThrowNInARow-1 > 0)
		{
			ThrowNInARow--;
			DesiredRotation.yaw = rotator( (playerHarry.Location - Location)*vect(1,1,0) ).yaw; //TurnToward(playerHarry);
			sleep( 0.3 );
			goto 'begin';
		}

		finishanim();
		DesiredRotation.yaw = rotator( playerHarry.Location - Location ).yaw;

		gotostate('PatrolForHarry');
}

//*************************************************************************************************************************
function ShootSpell()
{
	local  spellEcto   a;
	local  rotator     r;

	r = rotation;
	r.yaw = rotator(playerHarry.Location - Location).yaw;
	//a = spellEcto( SpawnSpell(class'spellEcto',playerHarry) );
	a = spellEcto( SpawnSpellEx( class'spellEcto', Location, r, playerHarry ) );
	a.fxFlyParticleEffect.ParticlesPerSec.Base *= 4;
	a.Velocity *= 0.35;
}

//*************************************************************************************************************************
/*
state stateHit
{
	ignores Trigger, TakeDamage;

	function bool RespondToDamage()   { return false;  }

  Begin:
	iNumHits++;

	if( iNumHits >= iNumHitsToBeat )
	{
		//PlayOuchSfx(false);
		PlayDialogLoud( "peeves_ow_long" );

		PlayAnim('prefly', 1.0, 0.3);
		FinishAnim();
		PlayAnim('idle', 1.0, 0.3);
		Sleep(1);
		loopAnim(StartAnim);
		
		//Start cut scene...
		SendDefeatedTrigger();

		//GotoState('stateIdle');
		playerHarry.StopBossEncounter();

		CleanupAfterBoss();

		SetPhysics( PHYS_Flying );

		GroundSpeed = 300;

		GotoState('stateIdle');
	}
	else // a normal hit
	{
		PlayOuchSfx(true);

		PlayAnim('prefly', 1.0, 0.3);

		if( bFlyBackToVPosSave )
			GotoState( 'stateHit2' );
		else
			GotoState( 'stateHit3' );
	}
}
*/
//*************************************************************************************************************************
state stateHit2
{
	function bool RespondToDamage()   { return false;  }

	function EndState()
	{
		DestroyControllers();
	}

	function OnEvent( name EventName )
	{
		if( EventName == 'ActionDone' )
			gotostate('stateHit3');
	}

  Begin:
	DoFlyTo( vPosSave, MOVE_TYPE_EASE_FROM_AND_TO, 0.5 );
}

//*************************************************************************************************************************
state stateHit3
{
	function bool FlyBackToVPosSave() { return true; }
	function bool RespondToDamage()   { return false;  }

	function BeginState()
	{
		TempRotator = rotation;
		TempRotator.pitch = 0;

		TempTime = Level.TimeSeconds;
		vPosSave = Location;
	}

	function Tick(float dtime)
	{
		local float    theta;
		local float    radius;
		local vector   v;
		local rotator  r;

		radius = 100;
		theta = (Level.TimeSeconds - TempTime) * 4.5;

		v.x = (-cos(theta) + 0.5) * radius;
		v.z = sin(theta) * radius;
		
		SetLocation( vPosSave + (v >> TempRotator) );
		r = TempRotator;
		r.Pitch = -theta * 32768.0 / 3.1415926;
		SetRotation( r );

		if( theta >= 2.0*3.1415926 )
		{
			SetRotation( TempRotator );

			//He speeds up, cause he's mad!
			MoveSpeed = GroundSpeed + (GroundSpeedEnd-GroundSpeed) * iNumHits/iNumHitsToBeat;
			if( playerHarry.Difficulty == DifficultyMedium )
				MoveSpeed *= 1.3;
			else
			if( playerHarry.Difficulty == DifficultyHard )
				MoveSpeed *= 1.8;

			PlayRunAnim();

			GotoState('PatrolForHarry');
		}
	}

  Begin:
	//FinishAnim();

	PlayHitSfx();

}

//*************************************************************************************************************************
function PlayOuchSfx(bool bPlayTalkingOuches)
{
	local sound  snd;
	local string str;

	//if( PlayNoSoundsTimer != 0 )
	//	return;

	bDoTakeDamageSpeech = true;

	if( !bPlayTalkingOuches  &&  iCurrentOuchSfx == 0 )
		iCurrentOuchSfx++; //just move to the next one, since there's only one right now.
	
	switch( iCurrentOuchSfx )
	{
		case 0:  	str = "peeves_ow01";     break;
		case 1:  	str = "peeves_ow02";     break;
		case 2:  	str = "peeves_ow03";     break;
	}

	PlayDialogLoud( str );
	PlayDialogLoud( "peeves_ow_long", SLOT_None, [Pitch]RandRange(0.8, 1.2) );

	iCurrentOuchSfx++;
	if( iCurrentOuchSfx > 2 )
		iCurrentOuchSfx = 0;
}

//*************************************************************************************************************************
function PlayHitSfx()
{
	local sound  snd;
	local string str;

	if( PlayNoSoundsTimer != 0 )
		return;

	if( !bDoTakeDamageSpeech )
		return;
/*
	switch( iHitSfx )
	{
		case 0:		FindDialog("Malfoy_005", snd, str);    break;
		case 1:		FindDialog("Malfoy_007", snd, str);     break;
		case 2:		FindDialog("125Malfoy Emotives2", snd, str);     break;
	}
*/
	PlaySound( snd );

	iHitSfx++;
	if( iHitSfx > 2 )
		iHitSfx = 0;

	PlayNoSoundsTimer = 3.0;  //dont play any more speech for a bit
}

//*************************************************************************************************************************
function PlayRandomSfx()
{
	local sound  snd;
	local string str;

	if( PlayNoSoundsTimer != 0 )
		return;

	if( FRand() < 0.75 )
		return;

/*
	switch( iRandomSfx )
	{
		case 0:		FindDialog("Malfoy_006", snd, str);     break;
		case 1:		FindDialog("MALFOY_003", snd, str);     break;
		case 2:		FindDialog("MALFOY_012", snd, str);     break;
		case 3:		FindDialog("112MalfoyInfo4", snd, str); break;
		case 4:		FindDialog("116MalfoyA4", snd, str);    break;
	}
*/
	PlaySound( snd );

	iRandomSfx++;
	if( iRandomSfx > 4 )
		iRandomSfx = 0;

	PlayNoSoundsTimer = 3.0;  //dont play any more speech for a bit
}

//*************************************************************************************************************************
function PlayMalfoyHitHarrySfx()
{
	local sound  snd;
	local string str;

	if( PlayNoSoundsTimer != 0 )
		return;

	if( playerHarry.HarryIsDead() )
		return;
/*
	switch( iMalfoyHitHarrySfx )
	{
		case 0:		FindDialog("125DracoMalfoy_Intro7", snd, str);   break;
		case 1:		FindDialog("MALFOY_008", snd, str);              break;
		case 2:		FindDialog("125Malfoy Emotives1", snd, str);     break;
		case 3:		FindDialog("MALFOY_009", snd, str);              break;
		case 4:		FindDialog("125Malfoy Emotives3", snd, str);     break;
		case 5:		FindDialog("MALFOY_010", snd, str);              break;
	}
*/
playerHarry.ClientMessage("iMalfoyHitHarrySfx:"$iMalfoyHitHarrySfx$"  snd:"$snd);
	PlaySound( snd );

	iMalfoyHitHarrySfx++;
	if( iMalfoyHitHarrySfx > 5 )
		iMalfoyHitHarrySfx = 0;
}

//*************************************************************************************************************************
function HarryWasHurt( bool bHarryWasKilled )
{
	if( PlayHarryHurtTimer == 0 )
		PlayHarryHurtTimer = 1.0;
}

//*************************************************************************************************************************
function CleanupAfterBoss()
{
	local actor a;

	foreach AllActors(SpellToCast, a)
		a.Destroy();

	//foreach allactors(class'actor', act, 'BossCollFightTag')
	//	act.SetCollision( false, false, false );
}

//*************************************************************************************************************************

defaultproperties
{
     SpellToCast=class'spellEcto'
     StartAnim=idle
     //Mesh=SkeletalMesh'skdracoMesh'
     DrawScale=1
	 iNumHitsToBeat=5

	Mesh=SkeletalMesh'HPModels.skpeevesMesh'
	DrawType=DT_Mesh
	Menuname="Peeves"
	eVulnerableToSpell=SPELL_Skurge
	Physics=PHYS_Rotating
	AmbientGlow=65
	CollisionRadius=40
    CollisionHeight=40

	//SightRadius=4000
	//PeripheralVision=0

	bCollideWorld=False
	bCollideActors=True
	bBlockActors=False
	bBlockPlayers=False
    bProjTarget=True
    RotationRate=(Pitch=80000,Yaw=80000,Roll=80000)
	//bGameOver=False

	airSpeed=150

	//fParticleTrailLife=1.0

	//HitDamage=20

    EnemyHealthBar=EnemyBar_Peeves

}
