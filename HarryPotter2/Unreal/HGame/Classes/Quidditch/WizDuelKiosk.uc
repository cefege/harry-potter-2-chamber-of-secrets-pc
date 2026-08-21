class WizDuelKiosk extends BlackboardWizDuel;

auto state idle
{
	event Bump(actor other)
	{
		if(harry(other)!=None && !HPConsole(Harry(level.playerHarryActor).player.console).menuBook.bIsOpen)
		{
			HPConsole(Harry(level.playerHarryActor).player.console).menuBook.OpenBook("DUEL");
		}
	}

}

defaultproperties
{
	collideType=CT_AlignedCylinder;
    CollisionRadius=70
    CollisionHeight=80
 
}
