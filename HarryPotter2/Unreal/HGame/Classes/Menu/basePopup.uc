class basePopup extends Actor;


var Harry  playerHarry;		// pointer to harry

function Draw (Canvas canvas)
{
}

function PostBeginPlay()
{
	//PlaySound(sound 'hpBase.events_sfx.s_writing2', SLOT_Interact, 1.0, false, 1000.0, 1.0);
	super.postBeginPlay();

	foreach allActors(class'Harry', playerHarry)
			break;

}

auto state popstate
{

	begin:

	sleep (0.5);
	playerHarry.clearMessages=false;

	poploop:
	if(playerHarry.clearMessages)
	{
		playerHarry.clearMessages=false;
		destroy();
	}
	sleep(0.03);
	goto 'poploop';


}


defaultproperties
{
	bHidden=True;
	lifespan=1.0;
}