class WarnTrigger extends trigger;

var() string WarningMessage;
var() float durration;

var Harry playerharry;

function PostBeginPlay()
{
	Super.PostBeginPlay();


	foreach allActors(class'Harry', playerharry)
	{
		break;
	}
}

event Trigger( Actor Other, Pawn EventInstigator )
{
	Touch(other);
}

function Touch( actor Other )
{
local actor A;
local Harry h;

	baseHUD(playerharry.myHUD).ShowPopup(class'basewarning');
	basewarning(baseHUD(playerharry.myHUD).curPopup).DisplayText = Localize( "all", WarningMessage,"Pickup" );
	basewarning(baseHUD(playerharry.myHUD).curPopup).lifespan=durration;

	if (bTriggerOnceOnly)
	{
		disable('Touch');
	}

}

defaultproperties
{
	TriggerType=TT_PlayerProximity
	durration=3.0;
}
