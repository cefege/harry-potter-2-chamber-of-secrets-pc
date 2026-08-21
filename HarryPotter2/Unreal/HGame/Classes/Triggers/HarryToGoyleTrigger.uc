
class HarryToGoyleTrigger extends trigger;

var   Harry   playerHarry;

var() class<ParticleFX>	Particles;
var() float				ParticlesWaitingTime;

//*******************************************************************************
function PostBeginPlay()
{
	ForEach AllActors(class'Harry', playerHarry)
		break;
}

//*******************************************************************************
function TriggerEvent( Name EventName, Actor Other, Pawn EventInstigator )
{
	gotostate('stateMagic');
}

state stateMagic
{
	begin:

		playerHarry.SpawnParticles(Particles);

		if(playerHarry.bIsGoyle)
			playerHarry.bIsGoyle = false;
		else
			playerHarry.bIsGoyle = true;

		Sleep(ParticlesWaitingTime);

		playerHarry.SetNewMesh();

		gotostate('NormalTrigger');
}

//*****************************************************************************
defaultproperties
{
	bSendEventOnEvent=true
	ParticlesWaitingTime=0.000000
}