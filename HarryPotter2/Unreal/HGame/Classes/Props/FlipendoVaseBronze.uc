//===============================================================================
//  [FlipendoVaseBronze] 
//===============================================================================

class FlipendoVaseBronze extends HFlipendo;

//var class<Actor> brokentype;
var class<Actor>  ShardType;
var Mesh   brokentypeMesh;

var bool   bBustedOnce;

auto state waitforspell
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		gotostate('break');
	}

	function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
	{
		local vector dir;
		local vector vel;
		local actor newspawn;
		local rotator newrot;

		//destroy();
			
		dir.x=20;
		dir.y=0;
		dir.z=0;
		dir=dir>>rotation;
		vel = dir * 4;
		dir=dir+location;

		playsound(sound'HPSounds.Hub1_sfx.vase_breaking');
		newspawn=fancyspawn(class'avifors_hit');//,,,location,rot(0,0,0));

		SetCollision(false,false,false);
		
		if( !bBustedOnce )
		{
			newSpawn=FancySpawn(transformInto);//,,,dir);

			if (newspawn.isa('jellybean'))
			{
				// Special case with beans, let them spill out		
				newSpawn.Velocity = vel;
			}
		}

		if( !bBustedOnce )
		{
			newrot=rotator( (Location - vHitLocation)*vect(1,1,0) );
			//newrot.yaw=newrot.yaw+32000;
			SetRotation( newrot );
			Mesh = brokentypeMesh;
			//newspawn=Fancyspawn(brokentype,,,location,newrot);

			bBustedOnce = true;
		}
		else
		{
			Destroy();
		}

		SetCollision(false,false,false);

		Fancyspawn(class'FlipendoVaseBronzeShard');//,,,dir,rotation);
		Fancyspawn(class'FlipendoVaseBronzeShard');//,,,dir,rotation);
		Fancyspawn(class'FlipendoVaseBronzeShard');//,,,dir,rotation);
		Fancyspawn(class'FlipendoVaseBronzeShard');//,,,dir,rotation);
		Fancyspawn(class'FlipendoVaseBronzeShard');//,,,dir,rotation);
		Fancyspawn(class'FlipendoVaseBronzeShard');//,,,dir,rotation);

		SetCollision(true,true,true);

		return true;
	}

}

defaultproperties
{
     DrawType=DT_Mesh
     CollisionRadius=15
     CollisionHeight=19
     Mesh=SkeletalMesh'HProps.skFlipendoVaseBronzeMesh'
	 eVulnerableToSpell=SPELL_Flipendo
	//brokentype=class'FlipendoVaseBronzeBroken';
	ShardType=class'FlipendoVaseBronzeShard'
	brokentypeMesh=SkeletalMesh'HProps.skFlipendoVaseBronzeBrokenMesh'

}
