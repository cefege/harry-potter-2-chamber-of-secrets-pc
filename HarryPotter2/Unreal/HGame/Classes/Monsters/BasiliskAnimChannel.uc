
class BasiliskAnimChannel expands AnimChannel;

var Basilisk Basil;
var bool     bAnimDone;

function _SetOwner( actor o )
{
	SetOwner( o );
	Basil = Basilisk(o);
}

function AnimEnd()
{
	bAnimDone = true;
	Basil.RealAnimEnd();
}

//Called from sk mesh anim
function PlayHissSound()
{
	Basil.PlayHissSound();
}