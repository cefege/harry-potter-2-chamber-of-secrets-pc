//=============================================================================
// QuidditchKiosk -- 
//=============================================================================
class QuidditchKiosk extends BlackboardQuidditch;


//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
}


auto state idle
{
	event Bump(actor other)
	{
		if(harry(other)!=None && !HPConsole(Harry(level.playerHarryActor).player.console).menuBook.bIsOpen)
		{
			HPConsole(Harry(level.playerHarryActor).player.console).menuBook.OpenBook("QUID");
//			gotostate('Active');
		}
	}

}

defaultproperties
{
	collideType=CT_AlignedCylinder;
}


//display