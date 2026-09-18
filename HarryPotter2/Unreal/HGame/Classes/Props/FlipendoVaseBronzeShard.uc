//===============================================================================
//  [FlipendoVaseBronzeShard] 
//===============================================================================

class FlipendoVaseBronzeShard extends HFlipendo;

var rotator randrot;
var bool tickOn;


auto state fall
{
	function tick(float deltaTime)
	{
		local vector loc;
		//loc=location;
		//if(tickOn)
		//{
		////	if(fasttrace(velocity*deltaTime))
		//		move(velocity*deltaTime);
		//	if(vsize(loc-location)>0.1)
		//	{
		//		velocity.z=velocity.z-(100*deltatime);
		//		setrotation(rotation+randrot);	
		//	}
		//}
	}

	function touch(actor other)
	{
		if(other==playerHarry)
			destroy();
	}

	function initfall()
	{
		local rotator randx;

		//randrot=rotrand();
		DesiredRotation = rotrand();

		RotationRate.yaw = Rand(40000);
		RotationRate.pitch = Rand(40000);
		RotationRate.roll = Rand(40000);

		randx.yaw = Rand(65536);//=rotation;
		//randx.yaw=randx.yaw+rand(20000);
		//randx.yaw=randx.yaw-10000;
		velocity=normal(vector(randx)) * RandRange(50,300);
		velocity.z=RandRange(80,300);	

	}

	begin:
	
		initfall();
		DrawScale = RandRange(1,4);
		tickOn=true;

	loop:
		sleep( RandRange(1.0,2.0) );
		tickon=false;
		//goto 'loop';
		destroy();
}


defaultproperties
{
    Mesh=skFlipendoVaseBronzeShardMesh
    DrawType=DT_Mesh
	bcollideworld=true
	bRotateToDesired=true
	bStatic=False
	Physics=PHYS_Falling
	drawscale=1
	collisionheight=1
	collisionradius=1
	tickOn=false

}

