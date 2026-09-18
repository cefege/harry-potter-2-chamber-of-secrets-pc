//===============================================================================

class chestbronze extends hprop;

const nMAX_EJECTED_OBJECTS = 8;

var()	int				iNumberOfBeans;
var()	class<Actor>	EjectedObjects[8]; // Make sure matches up with nMAX_EJECTED_OBJECTS
                                           // Up to 8 new objects can appear
                                           

var()	vector			ObjectStartPoint[8];
var()	vector			ObjectStartVelocity[8];
var()	bool			bRandomBeans;

// Persistence support
var     bool			bOpened;		// set to true once the chest has been opened.


function int GetMaxEjectedObjects()
{
	return (nMAX_EJECTED_OBJECTS);
}

function SetupRandomBeans()
{
	local int	iBean;

	for (iBean = 0; iBean < iNumberOfBeans; iBean ++)
	{
		if (rand(100) < (30 - (playerHarry.GetHealth() * 30)) && iBean == 0)
		{
			EjectedObjects[iBean] = Class'ChocolateFrog';
		}
		else
		{
			switch(rand(5))
			{
				case 0:
					EjectedObjects[iBean] = Class'BlueJellyBean';
					break;

				case 1:
					EjectedObjects[iBean] = Class'GreenJellyBean';
					break;

				case 2:
					EjectedObjects[iBean] = Class'SpottedJellyBean';
					break;

				case 3:
					EjectedObjects[iBean] = Class'GreenPurpleCheckerBean';
					break;

				case 4:
					EjectedObjects[iBean] = Class'RedBlackStripeBean';
					break;

			}
		}
	}
}

// Persistence support
state stillOpen
{
begin:
	//DEBUG
//	Level.playerHarryActor.ClientMessage(" State StillOpen for chest " $self );
	bProjTarget = false;
	eVulnerableToSpell = SPELL_None;
	loopanim('end');
}

auto state waitforspell
{
	function BeginState()
	{
		//DEBUG
//		Level.playerHarryActor.ClientMessage(" Chest " $self $" is wainting for spell with bOpened = " $bOpened );
		if( bOpened )
			gotostate( 'stillOpen' );
	}

	function bool HandleSpellAlohomora( optional baseSpell spell, optional vector vHitLocation )
	{
		local vector spawnLoc;
		local actor newSpawn;

		gotostate('turnover');
		return true;
	}


  begin:
	if( !bOpened )
	{
		//	SetPhysics(PHYS_walking);
		loopanim('start');
	}
}





state turnover
{
	function BeginState()
	{
		// for persistence
		bOpened = true;
		Level.playerHarryActor.ClientMessage(" Chest " $self $" is opening so bOpened = " $bOpened );
	}

	function generateobject()
	{
		
		local vector dir, vel;
		local actor newspawn;
		local rotator	SpawnDirection;
		local int	iBean;
		local rotator	HarryDirection, DifRotation;
		local bool   bPlayBeanSound;

		if (bRandomBeans)
		{
			SetupRandomBeans();
		}

		for (iBean = 0; iBean < iNumberOfBeans; iBean ++)
		{
			vel = ObjectStartVelocity[iBean];
			vel.x +=  (- 16 + rand(96));
			if (vel.x < 0)
			{
				vel.x = 0;
			}

			SpawnDirection = rotation;
			vel = vel >> SpawnDirection;

			dir = ObjectStartPoint[iBean];

			dir = dir >> SpawnDirection;
			dir = dir + location;

			newspawn=spawn(class'Spawn_flash_1',,,dir,rot(0,0,0));
			newSpawn=Spawn(EjectedObjects[iBean],,, dir);

			if (newspawn.isa('chocolatefrog'))
			{
				// Special case with beans, let them spill out		
				newSpawn.Velocity = vel * 2;
				bPlayBeanSound = true;
			}
			else if (newspawn.isa('wizardcardicon'))
			{
				// boost speed
				vel = ObjectStartVelocity[iBean];
				vel.x +=  20;

				SpawnDirection = rotation;
				vel = vel >> SpawnDirection;

				newSpawn.Velocity = vel;

				// Check for position of Harry and modify jump position accordingly

				HarryDirection = rotator(PlayerHarry.Location - dir);
				DifRotation = HarryDirection - SpawnDirection;

				DifRotation.yaw = DifRotation.yaw & 0xffff;
				if (DifRotation.yaw > 0x7fff)
					DifRotation.yaw -= 0x10000;

				if (abs(DifRotation.yaw) < 0x2000 && vsize(PlayerHarry.Location - dir) < 50)
				{
					if (DifRotation.yaw > 0)
						newSpawn.Velocity = newSpawn.Velocity << rot(0, 0x3800, 0);
					else
						newSpawn.Velocity = newSpawn.Velocity >> rot(0, 0x3800, 0);
				}

				newSpawn.SetLocation(Location + newSpawn.Velocity);
			}
			else  //everything else
			{
				newSpawn.Velocity = vel;
				bPlayBeanSound = true;
				newSpawn.SetPhysics( PHYS_Falling );
			}


			//Play appropriate sound

			if( bPlayBeanSound )
			{
				switch( Rand(3) )
				{
					case 0:   	PlaySound( sound'HPSounds.Magic_sfx.spawn_bean01');   break;
					case 1:   	PlaySound( sound'HPSounds.Magic_sfx.spawn_bean02');   break;
					case 2:   	PlaySound( sound'HPSounds.Magic_sfx.spawn_bean03');   break;
				}
			}
		}
	}

  begin:
	bProjTarget=false;
	eVulnerableToSpell = SPELL_None;

	// AE:
	//switch( Rand(4) )
	//{
	PlaySound( sound'HPSounds.General.wood_chest_open');
	//	case 0:	playsound(sound'HPSounds.Hub1_sfx.METAL_CHEST_OPEN_2'); break;
	//	case 1:	playsound(sound'HPSounds.Hub1_sfx.METAL_CHEST_OPEN_4'); break;
	//	case 2:	playsound(sound'HPSounds.Hub1_sfx.WOOD_CHEST_OPEN_1'); break;
	//	case 3:	playsound(sound'HPSounds.Hub1_sfx.WOOD_CHEST_OPEN_2'); break;
	//}

	playanim('open');
	finishanim();
	generateobject();
	//playsound(sound'HPSounds.Hub1_sfx.chest_landing');

	loopanim('end');
}

defaultproperties
{
     iNumberOfBeans=4
     EjectedObjects(0)=Class'HGame.Jellybean'
     EjectedObjects(1)=Class'HGame.Jellybean'
     EjectedObjects(2)=Class'HGame.Jellybean'
     EjectedObjects(3)=Class'HGame.Jellybean'
     EjectedObjects(4)=Class'HGame.Jellybean'
     EjectedObjects(5)=Class'HGame.Jellybean'
     EjectedObjects(6)=Class'HGame.Jellybean'
     EjectedObjects(7)=Class'HGame.Jellybean'
     ObjectStartPoint(0)=(Z=40)
     ObjectStartPoint(1)=(X=17,Y=-3,Z=40)
     ObjectStartPoint(2)=(X=2,Y=22,Z=40)
     ObjectStartPoint(3)=(X=-4,Y=-18,Z=40)
     ObjectStartPoint(4)=(X=22,Y=19,Z=40)
     ObjectStartPoint(5)=(X=16,Y=-23,Z=40)
     ObjectStartPoint(6)=(X=8,Y=36,Z=40)
     ObjectStartPoint(7)=(X=12,Y=-42,Z=40)
     ObjectStartVelocity(0)=(X=48,Z=120)
     ObjectStartVelocity(1)=(X=64,Z=200)
     ObjectStartVelocity(2)=(X=48,Y=24,Z=120)
     ObjectStartVelocity(3)=(X=48,Y=-24,Z=120)
     ObjectStartVelocity(4)=(X=64,Y=24,Z=200)
     ObjectStartVelocity(5)=(X=64,Y=-24,Z=200)
     ObjectStartVelocity(6)=(X=48,Y=48,Z=150)
     ObjectStartVelocity(7)=(X=48,Y=-48,Z=150)
     bRandomBeans=True
     Physics=PHYS_Falling
     eVulnerableToSpell=SPELL_Alohomora
     CentreOffset=(Z=20)
     Mesh=SkeletalMesh'HPModels.skbronzechestMesh'
     DrawScale=2
     CollisionRadius=24
     CollisionWidth=32
     CollisionHeight=24
     CollideType=CT_Box
}
