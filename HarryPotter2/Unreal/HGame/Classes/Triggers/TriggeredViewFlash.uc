//=============================================================================
// TriggeredViewFlash.
//=============================================================================
class TriggeredViewFlash expands Triggers;

//#exec TEXTURE IMPORT FILE=TriggeredViewFlash.pcx GROUP=System Mips=Off

var PlayerPawn Player;

function FIndPlayer()
{
	ForEach AllActors(class 'PlayerPawn', Player)
	{
		break;
	}

}


function Trigger(Actor Other, Pawn Instigator)
{
	if ( Player == none )
		FindPlayer();
	
	if ( Player != none )
		Player.ClientFlash( 1, vect(980, 808, 300));
}

defaultproperties
{
     Texture=Texture'HPEdit.Icons.flash'
}
